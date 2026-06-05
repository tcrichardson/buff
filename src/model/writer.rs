use crate::model::day::{Document, EntryTarget, Meeting};
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
