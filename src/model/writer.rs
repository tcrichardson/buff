use crate::model::day::{Document, EntryTarget, Meeting, Selectable, SelectableKind};
use crate::model::parser::{block_insert_index, heading_line, section_end};
use crate::model::day::SectionKind;

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
        let start = heading_line(&self.lines, SectionKind::Meetings)
            .expect("meetings section missing");
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

    pub fn add_entry(&mut self, target: &EntryTarget, text: &str, time: Option<&str>) {
        let bullet = match time {
            Some(t) => format!("- {} {}", t, text),
            None => format!("- {}", text),
        };

        match target {
            EntryTarget::Notes => {
                let start = heading_line(&self.lines, SectionKind::Notes)
                    .expect("notes section missing");
                let end = section_end(&self.lines, start);
                let insert_idx = block_insert_index(&self.lines, start, end);
                self.lines.insert(insert_idx, bullet);
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
                let insert_idx = block_insert_index(&self.lines, start, end);
                self.lines.insert(insert_idx, bullet);
            }
        }
    }

    pub fn add_todo(&mut self, text: &str, meeting_name: Option<&str>) {
        let start = heading_line(&self.lines, SectionKind::Todos)
            .expect("todos section missing");
        let end = section_end(&self.lines, start);
        let insert_idx = block_insert_index(&self.lines, start, end);
        let line = match meeting_name {
            Some(n) => format!("- [ ] {} _({})_", text, n),
            None => format!("- [ ] {}", text),
        };
        self.lines.insert(insert_idx, line);
    }

    pub fn selectables(&self) -> Vec<Selectable> {
        let mut result = Vec::new();
        for (i, line) in self.lines.iter().enumerate() {
            if let Some(rest) = line.strip_prefix("- [ ] ") {
                result.push(Selectable {
                    line: i,
                    kind: SelectableKind::Todo { done: false },
                    text: rest.to_string(),
                });
            } else if let Some(rest) = line.strip_prefix("- [x] ") {
                result.push(Selectable {
                    line: i,
                    kind: SelectableKind::Todo { done: true },
                    text: rest.to_string(),
                });
            } else if let Some(rest) = line.strip_prefix("- [X] ") {
                result.push(Selectable {
                    line: i,
                    kind: SelectableKind::Todo { done: true },
                    text: rest.to_string(),
                });
            } else if let Some(rest) = line.strip_prefix("- ") {
                result.push(Selectable {
                    line: i,
                    kind: SelectableKind::Entry,
                    text: rest.to_string(),
                });
            }
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
                let line = &self.lines[sel.line];
                let new_line = if done {
                    format!("- [ ] {}", &line[6..])
                } else {
                    format!("- [x] {}", &line[6..])
                };
                self.lines[sel.line] = new_line;
                Ok(())
            }
            SelectableKind::Entry => Err(anyhow::anyhow!("not a to-do")),
        }
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
        let mut doc =
            Document::from_text("# 2026-06-04\n\n## Meetings\n\n## Notes\n\n## To-dos\n");
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
        let mut doc =
            Document::from_text("# 2026-06-04\n\n## Meetings\n\n## Notes\n\n## To-dos\n");
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
        let mut doc =
            Document::from_text("# 2026-06-04\n\n## Meetings\n\n## Notes\n\n## To-dos\n");
        doc.add_todo("Renew cert", None);
        let text = doc.to_text();
        assert!(text.contains("## To-dos\n- [ ] Renew cert\n"), "got: {}", text);
    }

    #[test]
    fn add_todo_with_meeting_tag() {
        let mut doc =
            Document::from_text("# 2026-06-04\n\n## Meetings\n\n## Notes\n\n## To-dos\n");
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
        let mut doc =
            Document::from_text("# 2026-06-04\n\n## Meetings\n\n## Notes\n\n## To-dos\n");
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
    #[should_panic(expected = "todos section missing")]
    fn add_todo_panics_when_section_missing() {
        let mut doc = Document::from_text("# Title\n\n## Meetings\n\n## Notes\n");
        doc.add_todo("something", None);
    }

    #[test]
    fn selectables_over_spec_example() {
        let text = r"# 2026-06-04

## Meetings

### 09:15 Standup

- point A
- point B

### 10:00 Review

## Notes

- idea 1
- idea 2 _(tag)_

## To-dos

- [ ] unchecked todo
- [x] checked todo
- [ ] tagged todo _(meeting)_
- regular entry in todos
";
        let doc = Document::from_text(text);
        let sel = doc.selectables();
        assert_eq!(sel.len(), 8, "expected 8 selectables, got {:?}", sel);

        assert_eq!(sel[0].line, 6);
        assert_eq!(sel[0].kind, SelectableKind::Entry);
        assert_eq!(sel[0].text, "point A");

        assert_eq!(sel[1].line, 7);
        assert_eq!(sel[1].kind, SelectableKind::Entry);
        assert_eq!(sel[1].text, "point B");

        assert_eq!(sel[2].line, 13);
        assert_eq!(sel[2].kind, SelectableKind::Entry);
        assert_eq!(sel[2].text, "idea 1");

        assert_eq!(sel[3].line, 14);
        assert_eq!(sel[3].kind, SelectableKind::Entry);
        assert_eq!(sel[3].text, "idea 2 _(tag)_");

        assert_eq!(sel[4].line, 18);
        assert_eq!(sel[4].kind, SelectableKind::Todo { done: false });
        assert_eq!(sel[4].text, "unchecked todo");

        assert_eq!(sel[5].line, 19);
        assert_eq!(sel[5].kind, SelectableKind::Todo { done: true });
        assert_eq!(sel[5].text, "checked todo");

        assert_eq!(sel[6].line, 20);
        assert_eq!(sel[6].kind, SelectableKind::Todo { done: false });
        assert_eq!(sel[6].text, "tagged todo _(meeting)_");

        assert_eq!(sel[7].line, 21);
        assert_eq!(sel[7].kind, SelectableKind::Entry);
        assert_eq!(sel[7].text, "regular entry in todos");
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
        assert!(text.contains("- [ ] todo1\n"), "unchecked todo should be unchanged");
        assert!(text.contains("- [ ] todo2\n"), "checked todo should become unchecked");
    }
}
