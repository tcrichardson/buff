use crate::model::day::SectionKind;
use crate::model::day::{Document, EntryTarget, Meeting, Selectable, SelectableKind};
use crate::model::parser::{block_insert_index, ensure_section, heading_line, section_end};

/// True if the first line of an entry already looks like Markdown and should be
/// stored verbatim rather than wrapped in a bullet.
pub fn looks_like_markdown(first_line: &str) -> bool {
    let t = first_line.trim_start();
    if t.starts_with("```") {
        return true;
    }
    if t == ">" || t.starts_with("> ") {
        return true;
    }
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
        return true;
    }
    if crate::model::parser::heading_level(t).is_some() {
        return true;
    }
    crate::model::parser::is_ordered(t)
}

/// Convert composed (possibly multi-line) input into the Markdown lines to store.
/// Plain text becomes a bullet (with optional `HH:MM` timestamp on the first
/// line); anything that looks like Markdown is stored verbatim with no timestamp.
pub fn format_entry(input: &str, timestamp: Option<&str>) -> Vec<String> {
    let mut raw: Vec<&str> = input.split('\n').collect();
    while raw.len() > 1 && raw.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        raw.pop();
    }

    if looks_like_markdown(raw[0]) {
        return raw.iter().map(|s| s.to_string()).collect();
    }

    let mut out = Vec::with_capacity(raw.len());
    let first = match timestamp {
        Some(ts) => format!("- {} {}", ts, raw[0]),
        None => format!("- {}", raw[0]),
    };
    out.push(first);
    for line in &raw[1..] {
        out.push(format!("  {}", line));
    }
    out
}

impl Document {
    pub fn meetings(&self) -> Vec<Meeting> {
        let start = match heading_line(&self.lines, SectionKind::Meetings) {
            Some(i) => i,
            None => return Vec::new(),
        };
        let end = section_end(&self.lines, start);
        let mut meetings = Vec::new();
        for i in start + 1..end {
            let line = &self.lines[i];
            if let Some(rest) = line.strip_prefix("### ") {
                let mut parts = rest.splitn(2, ' ');
                let first = parts.next().unwrap_or("");
                let second = parts.next();
                let (time, name) = if let Some(name) = second {
                    if first.contains(':') {
                        (first.to_string(), name.to_string())
                    } else {
                        (String::new(), rest.to_string())
                    }
                } else {
                    (String::new(), first.to_string())
                };
                meetings.push(Meeting {
                    ordinal: meetings.len(),
                    heading_line: i,
                    time,
                    name,
                });
            }
        }
        meetings
    }

    pub fn add_meeting(&mut self, time: &str, name: &str) -> usize {
        let start = ensure_section(&mut self.lines, SectionKind::Meetings);
        let end = section_end(&self.lines, start);
        let insert_idx = block_insert_index(&self.lines, start, end);
        let line = if time.is_empty() {
            format!("### {}", name)
        } else {
            format!("### {} {}", time, name)
        };
        self.lines.insert(insert_idx, line);

        let mut ordinal = 0;
        for i in start + 1..insert_idx {
            if self.lines[i].starts_with("### ") {
                ordinal += 1;
            }
        }
        ordinal
    }

    pub fn add_block(&mut self, target: &EntryTarget, block: &[String]) {
        let insert_idx = match target {
            EntryTarget::Notes => {
                let start = ensure_section(&mut self.lines, SectionKind::Notes);
                let end = section_end(&self.lines, start);
                block_insert_index(&self.lines, start, end)
            }
            EntryTarget::Meeting(ord) => {
                let meetings = self.meetings();
                let meeting = meetings.get(*ord).expect("meeting not found");
                let start = meeting.heading_line;
                let end = self
                    .lines
                    .iter()
                    .enumerate()
                    .skip(start + 1)
                    .position(|(_, line)| line.starts_with("### ") || line.starts_with("## "))
                    .map(|i| start + 1 + i)
                    .unwrap_or(self.lines.len());
                block_insert_index(&self.lines, start, end)
            }
        };
        for (k, line) in block.iter().enumerate() {
            self.lines.insert(insert_idx + k, line.clone());
        }
    }

    pub fn add_entry(&mut self, target: &EntryTarget, text: &str, time: Option<&str>) {
        let bullet = match time {
            Some(t) => format!("- {} {}", t, text),
            None => format!("- {}", text),
        };
        self.add_block(target, &[bullet]);
    }

    pub fn add_todo(&mut self, text: &str, meeting_name: Option<&str>) {
        let start = ensure_section(&mut self.lines, SectionKind::Todos);
        let end = section_end(&self.lines, start);
        let insert_idx = block_insert_index(&self.lines, start, end);
        let line = match meeting_name {
            Some(n) => format!("- [ ] {} _({})_", text, n),
            None => format!("- [ ] {}", text),
        };
        self.lines.insert(insert_idx, line);
    }

    pub fn selectables(&self) -> Vec<Selectable> {
        use crate::model::parser::{
            continuation_end, heading_level, is_bullet, is_fence, is_ordered, is_quote,
            is_section_heading, todo_state,
        };

        let lines = &self.lines;
        let meetings_start = heading_line(lines, SectionKind::Meetings);
        let meetings_end = meetings_start.map(|s| section_end(lines, s));

        let mut result = Vec::new();
        let mut i = 0;
        let mut meeting_ord = 0usize;

        let join = |range: std::ops::Range<usize>| lines[range].join("\n");

        while i < lines.len() {
            let line = &lines[i];

            if line.trim().is_empty() {
                i += 1;
                continue;
            }
            // Structural headings (day title at line 0, fixed section headings) are not selectable.
            if (i == 0 && line.starts_with("# ")) || is_section_heading(line) {
                i += 1;
                continue;
            }

            // Code fence: run to the closing fence (or section end / EOF).
            if is_fence(line) {
                let start = i;
                let mut j = i + 1;
                while j < lines.len() && !is_fence(&lines[j]) && !is_section_heading(&lines[j]) {
                    j += 1;
                }
                let end = if j < lines.len() && is_fence(&lines[j]) {
                    j + 1
                } else {
                    j
                };
                result.push(Selectable {
                    lines: start..end,
                    kind: SelectableKind::CodeBlock,
                    text: join(start..end),
                });
                i = end;
                continue;
            }

            // Meeting heading inside the Meetings section.
            if line.starts_with("### ") {
                let in_meetings =
                    matches!((meetings_start, meetings_end), (Some(s), Some(e)) if i > s && i < e);
                if in_meetings {
                    result.push(Selectable {
                        lines: i..i + 1,
                        kind: SelectableKind::MeetingHeading {
                            ordinal: meeting_ord,
                        },
                        text: line.clone(),
                    });
                    meeting_ord += 1;
                    i += 1;
                    continue;
                }
            }

            // Markdown heading typed as a note.
            if heading_level(line).is_some() {
                result.push(Selectable {
                    lines: i..i + 1,
                    kind: SelectableKind::MarkdownHeading,
                    text: line.clone(),
                });
                i += 1;
                continue;
            }

            // Blockquote (consecutive quote lines).
            if is_quote(line) {
                let start = i;
                let mut j = i + 1;
                while j < lines.len() && is_quote(&lines[j]) {
                    j += 1;
                }
                result.push(Selectable {
                    lines: start..j,
                    kind: SelectableKind::Quote,
                    text: join(start..j),
                });
                i = j;
                continue;
            }

            // Todo (check before bullet, since "- [ ]" also matches "- ").
            if let Some(done) = todo_state(line) {
                let end = continuation_end(lines, i + 1);
                result.push(Selectable {
                    lines: i..end,
                    kind: SelectableKind::Todo { done },
                    text: join(i..end),
                });
                i = end;
                continue;
            }

            if is_bullet(line) {
                let end = continuation_end(lines, i + 1);
                result.push(Selectable {
                    lines: i..end,
                    kind: SelectableKind::Bullet,
                    text: join(i..end),
                });
                i = end;
                continue;
            }

            if is_ordered(line) {
                let end = continuation_end(lines, i + 1);
                result.push(Selectable {
                    lines: i..end,
                    kind: SelectableKind::Numbered,
                    text: join(i..end),
                });
                i = end;
                continue;
            }

            // Anything else: a single-line Raw block (e.g. external edits).
            result.push(Selectable {
                lines: i..i + 1,
                kind: SelectableKind::Raw,
                text: line.clone(),
            });
            i += 1;
        }

        result
    }

    pub fn toggle_todo(&mut self, sel_index: usize) -> anyhow::Result<()> {
        let selectables = self.selectables();
        let sel = selectables
            .get(sel_index)
            .ok_or_else(|| anyhow::anyhow!("index out of bounds"))?;
        match sel.kind {
            SelectableKind::Todo { done } => {
                let li = sel.lines.start;
                let content = &self.lines[li][6..];
                self.lines[li] = if done {
                    format!("- [ ] {}", content)
                } else {
                    format!("- [x] {}", content)
                };
                Ok(())
            }
            _ => Err(anyhow::anyhow!("not a to-do")),
        }
    }

    /// Replace the selected block's line range with `new_lines`.
    pub fn replace_selectable(
        &mut self,
        sel_index: usize,
        new_lines: &[String],
    ) -> anyhow::Result<()> {
        let selectables = self.selectables();
        let sel = selectables
            .get(sel_index)
            .ok_or_else(|| anyhow::anyhow!("index out of bounds"))?;
        let range = sel.lines.clone();
        self.lines.splice(range, new_lines.iter().cloned());
        Ok(())
    }

    pub fn delete_selectable(&mut self, sel_index: usize) -> anyhow::Result<()> {
        let selectables = self.selectables();
        let sel = selectables
            .get(sel_index)
            .ok_or_else(|| anyhow::anyhow!("index out of bounds"))?;
        let range = sel.lines.clone();
        self.lines.drain(range);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::day::SelectableKind;
    use chrono::NaiveDate;

    #[test]
    fn add_meeting_to_empty_doc() {
        let mut doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let ord = doc.add_meeting("09:15", "Standup");
        assert_eq!(ord, 0);
        let meetings = doc.meetings();
        assert_eq!(meetings.len(), 1);
        assert_eq!(meetings[0].ordinal, 0);
        assert_eq!(meetings[0].time, "09:15");
        assert_eq!(meetings[0].name, "Standup");
    }

    #[test]
    fn add_two_meetings() {
        let mut doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let ord0 = doc.add_meeting("09:15", "Standup");
        let ord1 = doc.add_meeting("10:00", "Review");
        assert_eq!(ord0, 0);
        assert_eq!(ord1, 1);
        let meetings = doc.meetings();
        assert_eq!(meetings.len(), 2);
        assert_eq!(meetings[0].name, "Standup");
        assert_eq!(meetings[1].name, "Review");
    }

    #[test]
    fn add_entry_to_notes() {
        let mut doc = Document::from_text("# 2026-06-04\n\n## Meetings\n\n## Notes\n\n## To-dos\n");
        doc.add_entry(&EntryTarget::Notes, "hi", None);
        let text = doc.to_text();
        assert!(text.contains("## Notes\n- hi\n"), "got: {}", text);
    }

    #[test]
    fn add_entry_to_meeting() {
        let mut doc = Document::from_text(
            "# 2026-06-04\n\n## Meetings\n\n### 09:15 Standup\n\n### 10:00 Review\n\n## Notes\n\n## To-dos\n",
        );
        doc.add_entry(&EntryTarget::Meeting(0), "point", None);
        let text = doc.to_text();
        let standup_pos = text.find("### 09:15 Standup").unwrap();
        let review_pos = text.find("### 10:00 Review").unwrap();
        let entry_pos = text.find("- point").unwrap();
        assert!(entry_pos > standup_pos, "entry should be after Standup");
        assert!(entry_pos < review_pos, "entry should be before Review");
    }

    #[test]
    fn add_entry_with_timestamp() {
        let mut doc = Document::from_text("# 2026-06-04\n\n## Meetings\n\n## Notes\n\n## To-dos\n");
        doc.add_entry(&EntryTarget::Notes, "point", Some("09:20"));
        let text = doc.to_text();
        assert!(text.contains("- 09:20 point\n"), "got: {}", text);
    }

    #[test]
    fn untouched_lines_preserved() {
        let mut doc = Document::from_text(
            "# 2026-06-04\n\n## Meetings\n\n### 09:15 Standup\n\n## Notes\n\n## To-dos\n\n- [ ] todo1\n",
        );
        doc.add_entry(&EntryTarget::Notes, "hi", None);
        let text = doc.to_text();

        let meetings_start = text.find("## Meetings").unwrap();
        let meetings_end = text.find("## Notes").unwrap();
        let meetings_section = &text[meetings_start..meetings_end];
        assert_eq!(meetings_section, "## Meetings\n\n### 09:15 Standup\n\n");

        let todos_start = text.find("## To-dos").unwrap();
        let todos_section = &text[todos_start..];
        assert_eq!(todos_section, "## To-dos\n\n- [ ] todo1\n");
    }

    #[test]
    fn add_todo_standalone() {
        let mut doc = Document::from_text("# 2026-06-04\n\n## Meetings\n\n## Notes\n\n## To-dos\n");
        doc.add_todo("Renew cert", None);
        let text = doc.to_text();
        assert!(
            text.contains("## To-dos\n- [ ] Renew cert\n"),
            "got: {}",
            text
        );
    }

    #[test]
    fn add_todo_with_meeting_tag() {
        let mut doc = Document::from_text("# 2026-06-04\n\n## Meetings\n\n## Notes\n\n## To-dos\n");
        doc.add_todo("Follow up", Some("Standup"));
        let text = doc.to_text();
        assert!(
            text.contains("## To-dos\n- [ ] Follow up _(Standup)_\n"),
            "got: {}",
            text
        );
    }

    #[test]
    fn add_todo_always_in_todos_section() {
        let mut doc = Document::from_text(
            "# 2026-06-04\n\n## Meetings\n\n### 09:15 Standup\n\n## Notes\n\n## To-dos\n",
        );
        doc.add_meeting("09:15", "Standup");
        doc.add_todo("Action item", None);
        let text = doc.to_text();

        let todos_start = text.find("## To-dos").unwrap();
        let todos_section = &text[todos_start..];
        assert!(todos_section.contains("- [ ] Action item"));
    }

    #[test]
    fn add_todo_ordering_preserved() {
        let mut doc = Document::from_text("# 2026-06-04\n\n## Meetings\n\n## Notes\n\n## To-dos\n");
        doc.add_todo("First", None);
        doc.add_todo("Second", None);
        doc.add_todo("Third", None);
        let text = doc.to_text();

        let first_pos = text.find("- [ ] First").unwrap();
        let second_pos = text.find("- [ ] Second").unwrap();
        let third_pos = text.find("- [ ] Third").unwrap();

        assert!(first_pos < second_pos, "First should come before Second");
        assert!(second_pos < third_pos, "Second should come before Third");
    }

    #[test]
    fn add_todo_creates_missing_section() {
        let mut doc = Document::from_text("# Title\n\n## Meetings\n\n## Notes\n");
        doc.add_todo("something", None);
        let text = doc.to_text();
        assert!(
            text.contains("## To-dos\n- [ ] something\n"),
            "got: {}",
            text
        );
    }

    #[test]
    fn add_entry_creates_missing_notes_section() {
        let mut doc = Document::from_text("# Title\n\n## Meetings\n\n## To-dos\n");
        doc.add_entry(&EntryTarget::Notes, "idea", None);
        let text = doc.to_text();
        assert!(text.contains("## Notes\n- idea\n"), "got: {}", text);
    }

    #[test]
    fn arbitrary_extra_lines_preserved() {
        let mut doc = Document::from_text(
            "# Title\n\nSome random prose here.\n\n## Meetings\n\n## Notes\n\n## To-dos\n\nMore prose at the end.\n",
        );
        doc.add_entry(&EntryTarget::Notes, "idea", None);
        let text = doc.to_text();
        assert!(
            text.contains("Some random prose here.\n"),
            "prose at top missing: {}",
            text
        );
        assert!(
            text.contains("More prose at the end.\n"),
            "prose at end missing: {}",
            text
        );
        assert!(text.contains("- idea\n"), "entry missing: {}", text);
    }

    #[test]
    fn add_meeting_creates_missing_meetings_section() {
        let mut doc = Document::from_text("# Title\n\n## Notes\n\n## To-dos\n");
        doc.add_meeting("09:15", "Standup");
        let text = doc.to_text();
        assert!(
            text.contains("## Meetings\n### 09:15 Standup\n"),
            "got: {}",
            text
        );
    }

    #[test]
    fn toggle_unchecked_todo() {
        let mut doc = Document::from_text("# 2026-06-04\n\n## To-dos\n\n- [ ] unchecked\n");
        doc.toggle_todo(0).unwrap();
        let text = doc.to_text();
        assert!(text.contains("- [x] unchecked\n"), "got: {}", text);
    }

    #[test]
    fn toggle_checked_todo() {
        let mut doc = Document::from_text("# 2026-06-04\n\n## To-dos\n\n- [x] checked\n");
        doc.toggle_todo(0).unwrap();
        let text = doc.to_text();
        assert!(text.contains("- [ ] checked\n"), "got: {}", text);
    }

    #[test]
    fn toggle_checked_todo_uppercase_x() {
        let mut doc = Document::from_text("# 2026-06-04\n\n## To-dos\n\n- [X] checked\n");
        doc.toggle_todo(0).unwrap();
        let text = doc.to_text();
        assert!(text.contains("- [ ] checked\n"), "got: {}", text);
    }

    #[test]
    fn toggle_entry_returns_err() {
        let mut doc = Document::from_text("# 2026-06-04\n\n## Notes\n\n- idea\n");
        let result = doc.toggle_todo(0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "not a to-do");
    }

    #[test]
    fn toggle_non_todo_lines_unchanged() {
        let mut doc = Document::from_text(
            "# 2026-06-04\n\n## Notes\n\n- idea\n\n## To-dos\n\n- [ ] todo1\n- [x] todo2\n",
        );
        doc.toggle_todo(2).unwrap(); // toggle the checked todo (index 2 in selectables)
        let text = doc.to_text();
        assert!(text.contains("- idea\n"), "notes entry should be unchanged");
        assert!(
            text.contains("- [ ] todo1\n"),
            "unchecked todo should be unchanged"
        );
        assert!(
            text.contains("- [ ] todo2\n"),
            "checked todo should become unchecked"
        );
    }

    #[test]
    fn delete_middle_selectable_removes_line() {
        let mut doc =
            Document::from_text("# 2026-06-04\n\n## Notes\n\n- first\n- second\n- third\n");
        doc.delete_selectable(1).unwrap();
        let text = doc.to_text();
        assert!(text.contains("- first\n"), "first should remain");
        assert!(!text.contains("- second\n"), "second should be removed");
        assert!(text.contains("- third\n"), "third should remain");
    }

    #[test]
    fn delete_updates_selectable_indices() {
        let mut doc =
            Document::from_text("# 2026-06-04\n\n## Notes\n\n- first\n- second\n- third\n");
        doc.delete_selectable(1).unwrap();
        let sel = doc.selectables();
        assert_eq!(sel.len(), 2);
        assert_eq!(sel[0].lines.start, 4);
        assert_eq!(sel[0].text, "- first");
        assert_eq!(sel[1].lines.start, 5);
        assert_eq!(sel[1].text, "- third");
    }

    #[test]
    fn delete_out_of_bounds_returns_err() {
        let mut doc = Document::from_text("# 2026-06-04\n\n## Notes\n\n- idea\n");
        let result = doc.delete_selectable(99);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "index out of bounds");
    }

    #[test]
    fn replace_selectable_swaps_lines() {
        let mut doc = Document::from_text("# 2026-06-04\n\n## Notes\n\n- old\n");
        doc.replace_selectable(0, &["> new".to_string()]).unwrap();
        let text = doc.to_text();
        assert!(text.contains("> new\n"), "got: {}", text);
        assert!(!text.contains("- old\n"), "old text should be gone");
    }

    #[test]
    fn replace_selectable_multiline_block() {
        let mut doc =
            Document::from_text("# 2026-06-04\n\n## Notes\n\n- first\n  cont\n\n- last\n");
        doc.replace_selectable(0, &["- replaced".to_string()])
            .unwrap();
        let text = doc.to_text();
        assert!(text.contains("- replaced\n"), "got: {}", text);
        assert!(!text.contains("  cont\n"), "continuation should be gone");
        assert!(text.contains("- last\n"), "last should remain");
    }

    #[test]
    fn replace_selectable_out_of_bounds_returns_err() {
        let mut doc = Document::from_text("# 2026-06-04\n\n## Notes\n\n- idea\n");
        let result = doc.replace_selectable(99, &["x".to_string()]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "index out of bounds");
    }

    #[test]
    fn replace_checked_todo_keeps_done_state() {
        let mut doc = Document::from_text("# Day\n\n## To-dos\n\n- [x] checked\n");
        let new_lines = crate::model::writer::format_entry("- [x] done", None);
        doc.replace_selectable(0, &new_lines).unwrap();
        let text = doc.to_text();
        assert!(text.contains("- [x] done\n"), "got: {}", text);
        assert!(!text.contains("- [x] checked\n"), "old text should be gone");
    }

    #[test]
    fn format_plain_single_line_becomes_bullet() {
        assert_eq!(format_entry("hello world", None), vec!["- hello world"]);
    }

    #[test]
    fn format_plain_single_line_with_timestamp() {
        assert_eq!(format_entry("hello", Some("09:20")), vec!["- 09:20 hello"]);
    }

    #[test]
    fn format_plain_multiline_indents_continuation() {
        assert_eq!(
            format_entry("first\nsecond\nthird", None),
            vec!["- first", "  second", "  third"]
        );
    }

    #[test]
    fn format_plain_multiline_timestamp_first_line_only() {
        assert_eq!(
            format_entry("first\nsecond", Some("09:20")),
            vec!["- 09:20 first", "  second"]
        );
    }

    #[test]
    fn format_heading_passthrough_verbatim() {
        assert_eq!(format_entry("## Section", None), vec!["## Section"]);
    }

    #[test]
    fn format_quote_passthrough_verbatim() {
        assert_eq!(format_entry("> a quote", Some("09:20")), vec!["> a quote"]);
    }

    #[test]
    fn format_ordered_list_passthrough_verbatim() {
        assert_eq!(format_entry("1. first", None), vec!["1. first"]);
    }

    #[test]
    fn format_explicit_bullet_passthrough_verbatim() {
        assert_eq!(format_entry("- already", None), vec!["- already"]);
    }

    #[test]
    fn format_code_fence_multiline_verbatim() {
        assert_eq!(
            format_entry("```rust\nfn main() {}\n```", None),
            vec!["```rust", "fn main() {}", "```"]
        );
    }

    #[test]
    fn format_strips_trailing_blank_lines() {
        assert_eq!(format_entry("hello\n", None), vec!["- hello"]);
    }

    #[test]
    fn looks_like_markdown_detects_signals() {
        assert!(looks_like_markdown("# h"));
        assert!(looks_like_markdown("###### h"));
        assert!(looks_like_markdown("> q"));
        assert!(looks_like_markdown("```"));
        assert!(looks_like_markdown("- b"));
        assert!(looks_like_markdown("* b"));
        assert!(looks_like_markdown("+ b"));
        assert!(looks_like_markdown("1. x"));
        assert!(looks_like_markdown("2) x"));
        assert!(!looks_like_markdown("plain text"));
        assert!(!looks_like_markdown("12.5 dollars"));
        assert!(!looks_like_markdown("#nospace"));
    }

    #[test]
    fn classify_blocks_full_example() {
        let text = "# 2026-06-04 (Thu)\n\n## Meetings\n\n### 09:15 Standup\n\n- point A\n  more A\n\n## Notes\n\n- idea\n> a quote\n1. one\n\n```rust\nfn x() {}\n```\n\n## To-dos\n\n- [ ] todo1\n- [x] todo2\n";
        let doc = Document::from_text(text);
        let sel = doc.selectables();

        let kinds: Vec<_> = sel
            .iter()
            .map(|s| (s.lines.clone(), s.kind.clone()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                (4..5, SelectableKind::MeetingHeading { ordinal: 0 }),
                (6..8, SelectableKind::Bullet), // "- point A" + "  more A"
                (11..12, SelectableKind::Bullet), // "- idea"
                (12..13, SelectableKind::Quote),
                (13..14, SelectableKind::Numbered),
                (15..18, SelectableKind::CodeBlock), // fence + body + fence
                (21..22, SelectableKind::Todo { done: false }),
                (22..23, SelectableKind::Todo { done: true }),
            ]
        );
        assert_eq!(sel[1].text, "- point A\n  more A");
        assert_eq!(sel[5].text, "```rust\nfn x() {}\n```");
    }

    #[test]
    fn classify_markdown_heading_in_notes_is_selectable() {
        let doc = Document::from_text("# Day\n\n## Notes\n\n## Subsection\n\n## To-dos\n");
        let sel = doc.selectables();
        assert_eq!(sel.len(), 1);
        assert_eq!(sel[0].kind, SelectableKind::MarkdownHeading);
        assert_eq!(sel[0].text, "## Subsection");
    }

    #[test]
    fn classify_unterminated_fence_runs_to_section_end() {
        let doc = Document::from_text("# Day\n\n## Notes\n\n```\nstuff\n\n## To-dos\n");
        let sel = doc.selectables();
        assert_eq!(sel.len(), 1);
        assert_eq!(sel[0].kind, SelectableKind::CodeBlock);
        assert_eq!(sel[0].lines, 4..7);
    }

    #[test]
    fn classify_raw_external_line_is_selectable() {
        let doc = Document::from_text("# Day\n\n## Notes\n\nplain external line\n\n## To-dos\n");
        let sel = doc.selectables();
        assert_eq!(sel.len(), 1);
        assert_eq!(sel[0].kind, SelectableKind::Raw);
        assert_eq!(sel[0].text, "plain external line");
    }

    #[test]
    fn add_block_inserts_multiple_lines_into_notes() {
        let mut doc = Document::from_text("# Day\n\n## Meetings\n\n## Notes\n\n## To-dos\n");
        doc.add_block(
            &EntryTarget::Notes,
            &["- one".to_string(), "  two".to_string()],
        );
        assert!(
            doc.to_text().contains("## Notes\n- one\n  two\n"),
            "got: {}",
            doc.to_text()
        );
    }
}
