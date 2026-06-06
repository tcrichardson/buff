use crate::config::Config;
use crate::model::day::{Document, Selectable};
use crate::storage;
use crate::ui::right_panel::{self, PanelTodo};
use chrono::NaiveDate;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Capture,
    Navigate,
    RightPanel,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Context {
    Notes,
    Meeting(usize),
    NoteBlock(usize),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Overlay {
    None,
    Calendar,
    Help,
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
    pub overlay: Overlay,
    pub editing: Option<usize>,
    pub should_quit: bool,
    pub selectables: Vec<Selectable>,
    pub context_display: String,
    pub pending_delete: bool,
    pub calendar: Option<crate::ui::calendar::CalendarState>,
    pub dates_with_notes: BTreeSet<NaiveDate>,
    pub right_panel_selected: usize,
    pub right_panel_scroll: usize,
    pub panel_todos: Vec<PanelTodo>,
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
        Ok(Self {
            doc,
            date,
            notes_dir,
            config,
            context: Context::Notes,
            focus: Focus::Capture,
            selected: 0,
            status: String::new(),
            input: String::new(),
            overlay: Overlay::None,
            editing: None,
            should_quit: false,
            selectables,
            context_display,
            pending_delete: false,
            calendar: None,
            dates_with_notes,
            right_panel_selected: 0,
            right_panel_scroll: 0,
            panel_todos,
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
        };
    }
}
