use crate::config::Config;
use crate::model::day::{Document, Selectable};
use crate::storage;
use crate::ui::right_panel::{self, PanelTodo};
use chrono::NaiveDate;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Capture,
    Navigate,
    RightPanel,
    Chat,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Context {
    Notes,
    Meeting(usize),
    NoteBlock(usize),
    Section { heading_line: usize, level: u8 },
    Todos,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Overlay {
    None,
    Help,
}

#[derive(Default)]
pub struct ChatState {
    pub visible: bool,
    pub messages: Vec<ChatMessage>,
    pub pending: bool,
    pub active_request: u64,
    pub scroll: usize,
    pub status: Option<String>,
    pub event_tx: Option<std::sync::mpsc::Sender<crate::app::llm::LlmEvent>>,
}

#[derive(Clone, Debug)]
pub struct UndoEntry {
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
}

#[derive(Clone, Debug, Default)]
pub struct VimState {
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub pending_op: Option<char>,
    pub yank_buffer: Vec<String>,
    pub undo_stack: Vec<UndoEntry>,
}

pub struct AppState {
    pub doc: Document,
    pub date: NaiveDate,
    pub notes_dir: PathBuf,
    pub config: Config,
    pub context: Context,
    pub focus: Focus,
    pub selected: usize,
    pub status: String,
    pub input: String,
    pub cursor_pos: usize,  // byte offset into `input`; always <= input.len(), always on a char boundary
    pub overlay: Overlay,
    pub editing: Option<usize>,
    pub should_quit: bool,
    pub selectables: Vec<Selectable>,
    pub context_display: String,
    pub pending_delete: bool,
    pub dates_with_notes: BTreeSet<NaiveDate>,
    pub right_panel_selected: usize,
    pub right_panel_scroll: usize, // scroll offset for todo list — scroll-follow not yet implemented
    pub panel_todos: Vec<PanelTodo>,
    pub chat: ChatState,
    pub vim: VimState,
}

impl AppState {
    pub fn open_day(notes_dir: PathBuf, config: Config, date: NaiveDate) -> anyhow::Result<Self> {
        let path = storage::path_for(&notes_dir, date, &config.date_format);
        let doc = if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            Document::from_text(&text)
        } else {
            Document::new_for_date(date)
        };
        let selectables = doc.selectables();
        let context_display = "context: Notes".to_string();
        let dates_with_notes = storage::dates_with_notes(&notes_dir, &config.date_format);
        let panel_todos = right_panel::collect_panel_todos(&notes_dir, date, &config);
        let chat_path = storage::chat_path_for(&notes_dir, date, &config.date_format);
        let chat_messages = storage::load_chat(&chat_path);
        Ok(Self {
            doc,
            date,
            notes_dir,
            config: config.clone(),
            context: Context::Notes,
            focus: Focus::Capture,
            selected: 0,
            status: String::new(),
            input: String::new(),
            cursor_pos: 0,
            overlay: Overlay::None,
            editing: None,
            should_quit: false,
            selectables,
            context_display,
            pending_delete: false,
            dates_with_notes,
            right_panel_selected: 0,
            right_panel_scroll: 0,
            panel_todos,
            chat: ChatState {
                visible: config.chat_visible,
                messages: chat_messages,
                ..Default::default()
            },
            vim: VimState::default(),
        })
    }

    pub fn save(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.notes_dir)?;
        let path = storage::path_for(&self.notes_dir, self.date, &self.config.date_format);
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.doc.to_text())?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn current_time_hhmm(&self) -> String {
        chrono::Local::now().format("%H:%M").to_string()
    }

    pub fn update_context_display(&mut self) {
        self.context_display = match self.context {
            Context::Notes => "context: Notes".to_string(),
            Context::Meeting(ord) => {
                let meetings = self.doc.meetings();
                match meetings.get(ord) {
                    Some(m) => format!("context: {}", m.name),
                    None => "context: Notes".to_string(),
                }
            }
            Context::NoteBlock(ord) => {
                let notes = self.doc.note_headings();
                match notes.get(ord) {
                    Some(n) => format!("context: {}", n.name),
                    None => "context: Notes".to_string(),
                }
            }
            Context::Section { heading_line, .. } => {
                let name = self.doc.lines
                    .get(heading_line)
                    .map(|l| l.trim_start_matches('#').trim_start())
                    .unwrap_or("section");
                format!("context: {}", name)
            }
            Context::Todos => "context: To-dos (use /todo to add)".to_string(),
        };
    }

    pub fn save_chat(&self) -> anyhow::Result<()> {
        let path = storage::chat_path_for(&self.notes_dir, self.date, &self.config.date_format);
        storage::save_chat(&path, &self.chat.messages)
    }

    pub fn handle_llm_event(&mut self, event: crate::app::llm::LlmEvent) {
        use crate::app::llm::LlmEvent;
        if event.id() != self.chat.active_request {
            return; // stale: superseded, cleared, or day switched
        }
        match event {
            LlmEvent::Started { .. } => {}
            LlmEvent::Token { text, .. } => {
                if let Some(last) = self.chat.messages.last_mut()
                    && last.role == ChatRole::Assistant
                {
                    last.content.push_str(&text);
                }
            }
            LlmEvent::Done { .. } => {
                self.chat.pending = false;
                let _ = self.save_chat();
            }
            LlmEvent::Error { message, .. } => {
                self.chat.pending = false;
                if matches!(
                    self.chat.messages.last(),
                    Some(m) if m.role == ChatRole::Assistant && m.content.is_empty()
                ) {
                    self.chat.messages.pop();
                }
                self.chat.status = Some(message);
                let _ = self.save_chat();
            }
        }
    }
}

/// Derive the editing context from a cursor position in the document.
/// Used to update `state.context` automatically as the vim cursor moves.
pub fn context_at_line(lines: &[String], cursor_line: usize) -> Context {
    if lines.is_empty() || cursor_line >= lines.len() {
        return Context::Notes;
    }

    // Step 1: walk backward to find the enclosing ## section boundary
    let boundary = match (0..=cursor_line).rev().find(|&i| lines[i].starts_with("## ")) {
        Some(b) => b,
        None => return Context::Notes,
    };

    let section = &lines[boundary];

    if section == "## To-dos" {
        return Context::Todos;
    }

    let in_meetings = section == "## Meetings";
    let in_notes = section == "## Notes";
    if !in_meetings && !in_notes {
        return Context::Notes;
    }

    // Step 2: walk forward from boundary to cursor_line tracking headings
    let mut l3_line: Option<usize> = None; // nearest ### heading
    let mut l4_line: Option<usize> = None; // nearest #### or deeper
    let mut l4_level: u8 = 0;

    for i in (boundary + 1)..=cursor_line {
        let line = &lines[i];
        if line.starts_with("## ") {
            break; // hit another top-level section — stop
        } else if line.starts_with("### ") {
            l3_line = Some(i);
            l4_line = None; // reset sub-section on new L3 heading
            l4_level = 0;
        } else if line.starts_with("#### ")
            || line.starts_with("##### ")
            || line.starts_with("###### ")
        {
            if l3_line.is_some() {
                l4_level = line.chars().take_while(|&c| c == '#').count() as u8;
                l4_line = Some(i);
            }
        }
    }

    // Step 3: return most specific context found
    if let Some(l4) = l4_line {
        return Context::Section { heading_line: l4, level: l4_level };
    }

    if let Some(l3) = l3_line {
        // Ordinal = number of ### headings from section start to l3, 0-indexed
        let ordinal = lines[(boundary + 1)..=l3]
            .iter()
            .filter(|l| l.starts_with("### "))
            .count()
            .saturating_sub(1);
        return if in_meetings {
            Context::Meeting(ordinal)
        } else {
            Context::NoteBlock(ordinal)
        };
    }

    Context::Notes
}

#[cfg(test)]
mod context_tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(|l| l.to_string()).collect()
    }

    #[test]
    fn cursor_above_all_sections_is_notes() {
        let ls = lines("# 2026-06-08 (Mon)\n");
        assert_eq!(context_at_line(&ls, 0), Context::Notes);
    }

    #[test]
    fn cursor_in_meetings_no_heading_is_notes() {
        let ls = lines("# Day\n\n## Meetings\n\n## Notes\n");
        // line 3 is blank inside ## Meetings
        assert_eq!(context_at_line(&ls, 3), Context::Notes);
    }

    #[test]
    fn cursor_on_meeting_heading_is_meeting_0() {
        let ls = lines("# Day\n\n## Meetings\n\n### Standup\n\n## Notes\n");
        // line 4 is "### Standup"
        assert_eq!(context_at_line(&ls, 4), Context::Meeting(0));
    }

    #[test]
    fn cursor_in_second_meeting_is_meeting_1() {
        let ls = lines(
            "# Day\n\n## Meetings\n\n### Standup\n- note\n\n### Design Review\n\n## Notes\n",
        );
        // line 7 is "### Design Review"
        assert_eq!(context_at_line(&ls, 7), Context::Meeting(1));
    }

    #[test]
    fn cursor_in_section_under_meeting() {
        let ls = lines("# Day\n\n## Meetings\n\n### Standup\n\n#### Action Items\n\n## Notes\n");
        // line 6 is "#### Action Items"
        assert_eq!(
            context_at_line(&ls, 6),
            Context::Section { heading_line: 6, level: 4 }
        );
    }

    #[test]
    fn cursor_in_todos_section() {
        let ls = lines("# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n\n- [ ] task\n");
        // line 8 is "- [ ] task"
        assert_eq!(context_at_line(&ls, 8), Context::Todos);
    }

    #[test]
    fn cursor_in_note_block() {
        let ls = lines("# Day\n\n## Meetings\n\n## Notes\n\n### My Note\n\n## To-dos\n");
        // line 6 is "### My Note"
        assert_eq!(context_at_line(&ls, 6), Context::NoteBlock(0));
    }

    #[test]
    fn cursor_on_empty_lines_vec() {
        assert_eq!(context_at_line(&[], 0), Context::Notes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_todos_display() {
        let tmp = tempfile::tempdir().unwrap();
        let mut s = AppState::open_day(
            tmp.path().to_path_buf(),
            Config::default(),
            NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
        )
        .unwrap();
        s.context = Context::Todos;
        s.update_context_display();
        assert!(
            s.context_display.contains("To-do"),
            "expected To-do in display, got: {}",
            s.context_display
        );
    }

    #[test]
    fn chat_message_json_roundtrip() {
        let msgs = vec![
            ChatMessage { role: ChatRole::User, content: "hi".to_string() },
            ChatMessage { role: ChatRole::Assistant, content: "hello".to_string() },
        ];
        let json = serde_json::to_string(&msgs).unwrap();
        let back: Vec<ChatMessage> = serde_json::from_str(&json).unwrap();
        assert_eq!(msgs, back);
        // role serializes lowercase
        assert!(json.contains("\"role\":\"user\""), "got: {}", json);
        assert!(json.contains("\"role\":\"assistant\""), "got: {}", json);
    }

    use crate::app::llm::LlmEvent;
    use crate::config::Config;
    use chrono::NaiveDate;

    fn chat_state_with(messages: Vec<ChatMessage>, active: u64, pending: bool) -> AppState {
        let tmp = tempfile::tempdir().unwrap();
        let mut s = AppState::open_day(
            tmp.path().to_path_buf(),
            Config::default(),
            NaiveDate::from_ymd_opt(2026, 6, 4).unwrap(),
        )
        .unwrap();
        s.chat.messages = messages;
        s.chat.active_request = active;
        s.chat.pending = pending;
        s
    }

    #[test]
    fn token_appends_to_last_assistant() {
        let mut s = chat_state_with(
            vec![
                ChatMessage { role: ChatRole::User, content: "q".into() },
                ChatMessage { role: ChatRole::Assistant, content: String::new() },
            ],
            5,
            true,
        );
        s.handle_llm_event(LlmEvent::Token { id: 5, text: "Hel".into() });
        s.handle_llm_event(LlmEvent::Token { id: 5, text: "lo".into() });
        assert_eq!(s.chat.messages.last().unwrap().content, "Hello");
    }

    #[test]
    fn stale_token_is_ignored() {
        let mut s = chat_state_with(
            vec![ChatMessage { role: ChatRole::Assistant, content: String::new() }],
            5,
            true,
        );
        s.handle_llm_event(LlmEvent::Token { id: 4, text: "nope".into() });
        assert_eq!(s.chat.messages.last().unwrap().content, "");
    }

    #[test]
    fn done_clears_pending() {
        let mut s = chat_state_with(
            vec![ChatMessage { role: ChatRole::Assistant, content: "hi".into() }],
            5,
            true,
        );
        s.handle_llm_event(LlmEvent::Done { id: 5 });
        assert!(!s.chat.pending);
    }

    #[test]
    fn error_before_tokens_removes_empty_placeholder_and_sets_status() {
        let mut s = chat_state_with(
            vec![
                ChatMessage { role: ChatRole::User, content: "q".into() },
                ChatMessage { role: ChatRole::Assistant, content: String::new() },
            ],
            5,
            true,
        );
        s.handle_llm_event(LlmEvent::Error { id: 5, message: "boom".into() });
        assert!(!s.chat.pending);
        assert_eq!(s.chat.messages.len(), 1); // empty assistant removed
        assert_eq!(s.chat.status.as_deref(), Some("boom"));
    }
}
