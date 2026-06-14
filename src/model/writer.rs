use crate::model::day::SectionKind;
use crate::model::day::{Document, EntryTarget, Meeting, Selectable, SelectableKind};
use crate::model::parser::{
    block_insert_index, ensure_section, heading_level, heading_line, section_end,
};

/// Strip leading `->` markers and replace each with two spaces.
/// `->` appearing anywhere other than the very start of the line is preserved.
pub fn expand_indent_markers(line: &str) -> String {
    let mut rest = line;
    let mut indent = String::new();
    while let Some(after) = rest.strip_prefix("->") {
        indent.push_str("  ");
        rest = after;
    }
    if indent.is_empty() {
        line.to_string()
    } else {
        format!("{}{}", indent, rest)
    }
}

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
/// Plain text is returned as-is (with optional `HH:MM` timestamp on the first
/// line); anything that looks like Markdown is stored verbatim with no timestamp.
pub fn format_entry(input: &str, timestamp: Option<&str>) -> Vec<String> {
    let mut raw: Vec<&str> = input.split('\n').collect();
    while raw.len() > 1 && raw.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        raw.pop();
    }

    // Expand leading `->` markers to spaces on every line before any other processing.
    let expanded: Vec<String> = raw.iter().map(|l| expand_indent_markers(l)).collect();
    let raw: Vec<&str> = expanded.iter().map(|s| s.as_str()).collect();

    if looks_like_markdown(raw[0]) {
        return raw.iter().map(|s| s.to_string()).collect();
    }

    let mut out = Vec::with_capacity(raw.len());
    let first = match timestamp {
        Some(ts) => format!("{} {}", ts, raw[0]),
        None => raw[0].to_string(),
    };
    out.push(first);
    for line in &raw[1..] {
        out.push(line.to_string());
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

    /// Returns `(scheduled_time, meeting_name)` pairs for all meetings that
    /// have a `Scheduled: HH:MM` metadata line immediately after the heading.
    /// Sorted by time ascending (lexicographic HH:MM sort is correct for 24h).
    pub fn meetings_with_scheduled(&self) -> Vec<(String, String)> {
        let mut result = Vec::new();
        for meeting in self.meetings() {
            for line in &self.lines[meeting.heading_line + 1..] {
                // Accept both new `meta:Scheduled: HH:MM` and legacy `Scheduled: HH:MM`.
                let value = line
                    .strip_prefix("meta:Scheduled: ")
                    .or_else(|| line.strip_prefix("Scheduled: "));
                if let Some(v) = value {
                    if !v.is_empty() {
                        result.push((v.to_string(), meeting.name.clone()));
                    }
                    break;
                }
                // Stop scanning if neither a meta: line nor a legacy time-field line.
                if !is_metadata_line(line) && !is_legacy_time_field_line(line) {
                    break;
                }
            }
        }
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    pub fn add_meeting(&mut self, name: &str) -> usize {
        let start = ensure_section(&mut self.lines, SectionKind::Meetings);
        let end = section_end(&self.lines, start);
        let insert_idx = block_insert_index(&self.lines, start, end);
        let line = format!("### {}", name);
        self.lines.insert(insert_idx, line);

        let mut ordinal = 0;
        for i in start + 1..insert_idx {
            if self.lines[i].starts_with("### ") {
                ordinal += 1;
            }
        }
        ordinal
    }

    pub fn add_note_heading(&mut self, name: &str) -> usize {
        let start = ensure_section(&mut self.lines, SectionKind::Notes);
        let end = section_end(&self.lines, start);
        let insert_idx = block_insert_index(&self.lines, start, end);
        let line = format!("### {}", name);
        self.lines.insert(insert_idx, line);

        let mut ordinal = 0;
        for i in start + 1..insert_idx {
            if self.lines[i].starts_with("### ") {
                ordinal += 1;
            }
        }
        ordinal
    }

    pub fn note_headings(&self) -> Vec<Meeting> {
        let start = match heading_line(&self.lines, SectionKind::Notes) {
            Some(i) => i,
            None => return Vec::new(),
        };
        let end = section_end(&self.lines, start);
        let mut notes = Vec::new();
        for i in start + 1..end {
            let line = &self.lines[i];
            if let Some(rest) = line.strip_prefix("### ") {
                notes.push(Meeting {
                    ordinal: notes.len(),
                    heading_line: i,
                    time: String::new(),
                    name: rest.to_string(),
                });
            }
        }
        notes
    }

    /// Returns the insertion index for `Meeting`, `NoteBlock`, and `Section` targets.
    /// Panics if called with `EntryTarget::Notes` — that case requires `ensure_section` first.
    fn insertion_index_for_target(&self, target: &EntryTarget) -> usize {
        match target {
            EntryTarget::Notes => panic!("Notes target requires ensure_section; use add_block"),
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
            EntryTarget::NoteBlock(ord) => {
                let notes = self.note_headings();
                let note = notes.get(*ord).expect("note not found");
                let start = note.heading_line;
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
            EntryTarget::Section {
                heading_line,
                level,
            } => {
                let start = *heading_line;
                let end = self
                    .lines
                    .iter()
                    .enumerate()
                    .skip(start + 1)
                    .position(|(_, line)| {
                        heading_level(line).map_or(false, |lv| lv <= *level as usize)
                    })
                    .map(|i| start + 1 + i)
                    .unwrap_or(self.lines.len());
                block_insert_index(&self.lines, start, end)
            }
        }
    }

    pub fn add_block(&mut self, target: &EntryTarget, block: &[String]) {
        let insert_idx = match target {
            EntryTarget::Notes => {
                let start = ensure_section(&mut self.lines, SectionKind::Notes);
                let end = section_end(&self.lines, start);
                block_insert_index(&self.lines, start, end)
            }
            other => self.insertion_index_for_target(other),
        };
        for (k, line) in block.iter().enumerate() {
            self.lines.insert(insert_idx + k, line.clone());
        }
    }

    /// Insert a heading of `level` hashes with the given `name` at the end of `target`'s
    /// content, and return the line index of the newly inserted heading.
    /// The returned index is stable: subsequent insertions always go *after* the heading,
    /// so it never shifts.
    pub fn add_section_heading(&mut self, target: &EntryTarget, level: u8, name: &str) -> usize {
        let insert_idx = self.insertion_index_for_target(target);
        let hashes = "#".repeat(level as usize);
        self.lines
            .insert(insert_idx, format!("{} {}", hashes, name));
        insert_idx
    }

    /// Append an AI-generated meeting summary at the end of the given meeting's
    /// section. If a `#### Meeting Summary` block already exists, it is replaced.
    /// The summary is normalized to ensure it starts with `#### Meeting Summary`.
    pub fn add_meeting_summary(&mut self, meeting_ordinal: usize, summary: &str) {
        let meetings = self.meetings();
        let Some(meeting) = meetings.get(meeting_ordinal) else {
            return;
        };
        let heading_line = meeting.heading_line;

        // Exclusive end index of this meeting's section.
        let section_end = self.lines[heading_line + 1..]
            .iter()
            .position(|line| line.starts_with("### ") || line.starts_with("## "))
            .map(|i| heading_line + 1 + i)
            .unwrap_or(self.lines.len());

        // Check if an existing "#### Meeting Summary" is already present.
        let existing_start = self.lines[heading_line..section_end]
            .iter()
            .position(|line| line == "#### Meeting Summary")
            .map(|i| heading_line + i);

        let summary_lines: Vec<String> = normalize_summary(summary)
            .lines()
            .map(|l| l.to_string())
            .collect();

        if let Some(start) = existing_start {
            // Replace from the existing heading to the section end.
            self.lines.splice(start..section_end, summary_lines);
        } else {
            // Insert after the last non-blank content line in the section.
            let mut insert_at = section_end;
            while insert_at > heading_line + 1
                && self
                    .lines
                    .get(insert_at - 1)
                    .map_or(false, |l| l.trim().is_empty())
            {
                insert_at -= 1;
            }

            let mut to_insert = Vec::new();
            // Add a blank separator if the preceding line is non-blank.
            if self
                .lines
                .get(insert_at.saturating_sub(1))
                .map_or(false, |l| !l.trim().is_empty())
            {
                to_insert.push(String::new());
            }
            to_insert.extend(summary_lines);

            self.lines.splice(insert_at..insert_at, to_insert);
        }
    }

    pub fn add_entry(&mut self, target: &EntryTarget, text: &str, time: Option<&str>) {
        let block = format_entry(text, time);
        self.add_block(target, &block);
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
        let notes_start = heading_line(lines, SectionKind::Notes);
        let notes_end = notes_start.map(|s| section_end(lines, s));

        let mut result = Vec::new();
        let mut i = 0;
        let mut meeting_ord = 0usize;
        let mut note_ord = 0usize;

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
                let in_notes =
                    matches!((notes_start, notes_end), (Some(s), Some(e)) if i > s && i < e);
                if in_notes {
                    result.push(Selectable {
                        lines: i..i + 1,
                        kind: SelectableKind::NoteHeading { ordinal: note_ord },
                        text: line.clone(),
                    });
                    note_ord += 1;
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

    /// Toggle the todo on a specific line by raw line index.
    /// Returns Ok(()) if the line was a todo, Err if it was not or index is out of bounds.
    pub fn toggle_todo_at_line(&mut self, line_idx: usize) -> anyhow::Result<()> {
        let line = self
            .lines
            .get(line_idx)
            .ok_or_else(|| anyhow::anyhow!("line index {} out of bounds", line_idx))?;
        if line.starts_with("- [ ] ") {
            let rest = line[6..].to_string();
            self.lines[line_idx] = format!("- [x] {}", rest);
            Ok(())
        } else if line.starts_with("- [x] ") || line.starts_with("- [X] ") {
            let rest = line[6..].to_string();
            self.lines[line_idx] = format!("- [ ] {}", rest);
            Ok(())
        } else {
            Err(anyhow::anyhow!("line {} is not a todo", line_idx))
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

const METADATA_FIELD_ORDER: &[&str] = &["Purpose", "Topic", "Scheduled", "Started", "Ended"];

/// Legacy bare time-field keys written before the `meta:` prefix was introduced.
const LEGACY_TIME_KEYS: &[&str] = &["Scheduled", "Started", "Ended"];

/// True if the line begins with the `meta:` storage prefix that identifies a metadata line.
fn is_metadata_line(line: &str) -> bool {
    line.starts_with("meta:")
}

/// True if the line is a legacy (pre-migration) bare time-field: `Scheduled: `, `Started: `, `Ended: `.
fn is_legacy_time_field_line(line: &str) -> bool {
    LEGACY_TIME_KEYS
        .iter()
        .any(|k| line.starts_with(&format!("{}: ", k)))
}

/// Insert or replace a metadata field (`meta:Key: value`) in the block immediately
/// after the heading at `heading_line`.
///
/// The metadata block is any consecutive run of `meta:` lines (or legacy bare
/// time-field lines, which are transparently migrated to `meta:` format on write).
/// The block is always rewritten in the canonical order defined by `METADATA_FIELD_ORDER`.
pub fn set_metadata_field(lines: &mut Vec<String>, heading_line: usize, key: &str, value: &str) {
    // Find the end of the existing metadata block (supports both formats).
    let mut meta_end = heading_line + 1;
    while meta_end < lines.len()
        && (is_metadata_line(&lines[meta_end]) || is_legacy_time_field_line(&lines[meta_end]))
    {
        meta_end += 1;
    }

    // Parse existing fields into a map, stripping `meta:` prefix when present.
    let mut fields: std::collections::HashMap<String, String> = lines[heading_line + 1..meta_end]
        .iter()
        .filter_map(|line| {
            let data = line.strip_prefix("meta:").unwrap_or(line.as_str());
            let mut parts = data.splitn(2, ": ");
            let k = parts.next()?.to_string();
            let v = parts.next()?.to_string();
            Some((k, v))
        })
        .collect();

    // Insert or overwrite the target key.
    fields.insert(key.to_string(), value.to_string());

    // Rebuild in canonical order with `meta:` prefix (only keys that are present).
    let new_lines: Vec<String> = METADATA_FIELD_ORDER
        .iter()
        .filter_map(|k| fields.get(*k).map(|v| format!("meta:{}: {}", k, v)))
        .collect();

    // Replace the old metadata range with the new lines.
    lines.drain(heading_line + 1..meta_end);
    for (i, line) in new_lines.into_iter().enumerate() {
        lines.insert(heading_line + 1 + i, line);
    }
}

fn normalize_summary(summary: &str) -> String {
    let summary = summary.trim();
    if summary.starts_with("#### Meeting Summary") {
        summary.to_string()
    } else {
        format!("#### Meeting Summary\n{}", summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::day::SelectableKind;
    use chrono::NaiveDate;

    #[test]
    fn add_note_heading_to_empty_doc() {
        let mut doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let ord = doc.add_note_heading("Idea Bucket");
        assert_eq!(ord, 0);
        let notes = doc.note_headings();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].ordinal, 0);
        assert_eq!(notes[0].name, "Idea Bucket");
    }

    #[test]
    fn add_two_note_headings() {
        let mut doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let ord0 = doc.add_note_heading("First");
        let ord1 = doc.add_note_heading("Second");
        assert_eq!(ord0, 0);
        assert_eq!(ord1, 1);
        let notes = doc.note_headings();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].name, "First");
        assert_eq!(notes[1].name, "Second");
    }

    #[test]
    fn add_entry_to_note_block() {
        let mut doc = Document::from_text(
            "# 2026-06-04\n\n## Meetings\n\n## Notes\n\n### Idea Bucket\n\n## To-dos\n",
        );
        doc.add_entry(&EntryTarget::NoteBlock(0), "point", None);
        let text = doc.to_text();
        let heading_pos = text.find("### Idea Bucket").unwrap();
        let entry_pos = text.find("point\n").unwrap();
        assert!(
            entry_pos > heading_pos,
            "entry should be after note heading"
        );
    }

    #[test]
    fn note_heading_is_selectable() {
        let doc = Document::from_text("# Day\n\n## Notes\n\n### Idea Bucket\n\n## To-dos\n");
        let sel = doc.selectables();
        assert_eq!(sel.len(), 1);
        assert_eq!(sel[0].kind, SelectableKind::NoteHeading { ordinal: 0 });
        assert_eq!(sel[0].text, "### Idea Bucket");
    }

    #[test]
    fn add_meeting_to_empty_doc() {
        let mut doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let ord = doc.add_meeting("Standup");
        assert_eq!(ord, 0);
        let meetings = doc.meetings();
        assert_eq!(meetings.len(), 1);
        assert_eq!(meetings[0].ordinal, 0);
        assert_eq!(meetings[0].time, "");
        assert_eq!(meetings[0].name, "Standup");
    }

    #[test]
    fn add_two_meetings() {
        let mut doc = Document::new_for_date(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
        let ord0 = doc.add_meeting("Standup");
        let ord1 = doc.add_meeting("Review");
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
        assert!(text.contains("## Notes\nhi\n"), "got: {}", text);
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
        let entry_pos = text.find("point\n").unwrap();
        assert!(entry_pos > standup_pos, "entry should be after Standup");
        assert!(entry_pos < review_pos, "entry should be before Review");
    }

    #[test]
    fn add_entry_with_timestamp() {
        let mut doc = Document::from_text("# 2026-06-04\n\n## Meetings\n\n## Notes\n\n## To-dos\n");
        doc.add_entry(&EntryTarget::Notes, "point", Some("09:20"));
        let text = doc.to_text();
        assert!(text.contains("09:20 point\n"), "got: {}", text);
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
        doc.add_meeting("Standup");
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
    fn add_entry_creates_missing_notes_section() {
        let mut doc = Document::from_text("# Title\n\n## Meetings\n\n## To-dos\n");
        doc.add_entry(&EntryTarget::Notes, "idea", None);
        let text = doc.to_text();
        assert!(text.contains("## Notes\nidea\n"), "got: {}", text);
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
        assert!(text.contains("idea\n"), "entry missing: {}", text);
    }

    #[test]
    fn add_meeting_creates_missing_meetings_section() {
        let mut doc = Document::from_text("# Title\n\n## Notes\n\n## To-dos\n");
        doc.add_meeting("Standup");
        let text = doc.to_text();
        assert!(text.contains("## Meetings\n### Standup\n"), "got: {}", text);
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
    fn format_plain_single_line_passthrough() {
        assert_eq!(format_entry("hello world", None), vec!["hello world"]);
    }

    #[test]
    fn format_plain_single_line_with_timestamp() {
        assert_eq!(format_entry("hello", Some("09:20")), vec!["09:20 hello"]);
    }

    #[test]
    fn format_plain_multiline_passthrough() {
        assert_eq!(
            format_entry("first\nsecond\nthird", None),
            vec!["first", "second", "third"]
        );
    }

    #[test]
    fn format_plain_multiline_timestamp_first_line_only() {
        assert_eq!(
            format_entry("first\nsecond", Some("09:20")),
            vec!["09:20 first", "second"]
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
        assert_eq!(format_entry("hello\n", None), vec!["hello"]);
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

    #[test]
    fn indented_bullet_is_independent_selectable() {
        let doc = Document::from_text("# Day\n\n## Notes\n\n- parent\n  - child\n\n## To-dos\n");
        let sel = doc.selectables();
        assert_eq!(sel.len(), 2, "expected two selectables, got: {:?}", sel);
        assert_eq!(sel[0].kind, SelectableKind::Bullet);
        assert_eq!(sel[0].text, "- parent");
        assert_eq!(sel[1].kind, SelectableKind::Bullet);
        assert_eq!(sel[1].text, "  - child");
    }

    #[test]
    fn indented_todo_is_independent_selectable() {
        let doc =
            Document::from_text("# Day\n\n## Notes\n\n- parent\n  - [ ] child\n\n## To-dos\n");
        let sel = doc.selectables();
        assert_eq!(sel.len(), 2, "expected two selectables, got: {:?}", sel);
        assert_eq!(sel[0].kind, SelectableKind::Bullet);
        assert_eq!(sel[0].text, "- parent");
        assert_eq!(sel[1].kind, SelectableKind::Todo { done: false });
        assert_eq!(sel[1].text, "  - [ ] child");
    }

    #[test]
    fn parent_with_continuation_then_sub_bullet() {
        let doc =
            Document::from_text("# Day\n\n## Notes\n\n- parent\n  cont\n  - child\n\n## To-dos\n");
        let sel = doc.selectables();
        assert_eq!(sel.len(), 2, "expected two selectables, got: {:?}", sel);
        assert_eq!(sel[0].kind, SelectableKind::Bullet);
        assert_eq!(sel[0].text, "- parent\n  cont");
        assert_eq!(sel[1].kind, SelectableKind::Bullet);
        assert_eq!(sel[1].text, "  - child");
    }

    #[test]
    fn replace_sub_bullet_does_not_join_parent() {
        let mut doc =
            Document::from_text("# Day\n\n## Notes\n\n- parent\n  - child\n\n## To-dos\n");
        let sel = doc.selectables();
        let child_idx = sel
            .iter()
            .position(|s| s.text == "  - child")
            .expect("child should be selectable");
        doc.replace_selectable(child_idx, &["  - changed".to_string()])
            .unwrap();
        let text = doc.to_text();
        assert!(
            text.contains("- parent\n"),
            "parent should remain: {}",
            text
        );
        assert!(
            text.contains("  - changed\n"),
            "child should be changed: {}",
            text
        );
        assert!(
            !text.contains("  - child\n"),
            "old child should be gone: {}",
            text
        );
    }

    #[test]
    fn set_metadata_field_inserts_started_when_absent() {
        let mut doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\n- note\n\n## Notes\n\n## To-dos\n",
        );
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Started", "09:15");
        let text = doc.to_text();
        let heading_pos = text.find("### Standup").unwrap();
        let started_pos = text.find("meta:Started: 09:15").unwrap();
        let note_pos = text.find("- note").unwrap();
        assert!(started_pos > heading_pos, "Started should be after heading");
        assert!(started_pos < note_pos, "Started should be before note");
    }

    #[test]
    fn set_metadata_field_overwrites_existing_started() {
        let mut doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\nmeta:Started: 09:00\n- note\n\n## Notes\n\n## To-dos\n",
        );
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Started", "09:15");
        let text = doc.to_text();
        assert!(
            text.contains("meta:Started: 09:15\n"),
            "should have new time: {}",
            text
        );
        assert!(
            !text.contains("meta:Started: 09:00\n"),
            "old time should be gone: {}",
            text
        );
    }

    #[test]
    fn set_metadata_field_canonical_order() {
        // Add Ended first, then Started, then Scheduled — result should be Purpose, Scheduled, Started, Ended
        let mut doc =
            Document::from_text("# Day\n\n## Meetings\n\n### Standup\n\n## Notes\n\n## To-dos\n");
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Ended", "10:00");
        set_metadata_field(&mut doc.lines, heading, "Started", "09:15");

        // re-fetch heading_line since lines shifted
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Scheduled", "09:00");

        let text = doc.to_text();
        let scheduled_pos = text.find("meta:Scheduled: 09:00").unwrap();
        let started_pos = text.find("meta:Started: 09:15").unwrap();
        let ended_pos = text.find("meta:Ended: 10:00").unwrap();
        assert!(scheduled_pos < started_pos, "Scheduled before Started");
        assert!(started_pos < ended_pos, "Started before Ended");
    }

    #[test]
    fn set_metadata_field_does_not_eat_note_content() {
        let mut doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\n- note one\n- note two\n\n## Notes\n\n## To-dos\n",
        );
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Started", "09:15");
        let text = doc.to_text();
        assert!(
            text.contains("- note one\n"),
            "note one should remain: {}",
            text
        );
        assert!(
            text.contains("- note two\n"),
            "note two should remain: {}",
            text
        );
    }

    #[test]
    fn set_metadata_field_all_three_fields() {
        let mut doc =
            Document::from_text("# Day\n\n## Meetings\n\n### Standup\n\n## Notes\n\n## To-dos\n");
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Scheduled", "09:00");
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Started", "09:05");
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Ended", "09:45");
        let text = doc.to_text();
        assert!(text.contains("meta:Scheduled: 09:00\n"), "got: {}", text);
        assert!(text.contains("meta:Started: 09:05\n"), "got: {}", text);
        assert!(text.contains("meta:Ended: 09:45\n"), "got: {}", text);
    }

    #[test]
    fn is_metadata_line_recognizes_meta_prefix() {
        assert!(is_metadata_line("meta:Scheduled: 09:00"));
        assert!(is_metadata_line("meta:Purpose: kick off Q3"));
        assert!(!is_metadata_line("Scheduled: 09:00"));
        assert!(!is_metadata_line("- bullet"));
        assert!(!is_metadata_line(""));
    }

    #[test]
    fn set_metadata_field_inserts_purpose_when_absent() {
        let mut doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\n- note\n\n## Notes\n\n## To-dos\n",
        );
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Purpose", "align team");
        let text = doc.to_text();
        assert!(text.contains("meta:Purpose: align team\n"), "got: {}", text);
        let purpose_pos = text.find("meta:Purpose:").unwrap();
        let note_pos = text.find("- note").unwrap();
        assert!(purpose_pos < note_pos, "Purpose should precede body");
    }

    #[test]
    fn set_metadata_field_inserts_topic_in_note_block() {
        let mut doc = Document::from_text("# Day\n\n## Notes\n\n### Design\n- note\n\n## To-dos\n");
        let heading = doc.note_headings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Topic", "API v2");
        let text = doc.to_text();
        assert!(text.contains("meta:Topic: API v2\n"), "got: {}", text);
    }

    #[test]
    fn set_metadata_field_overwrites_existing() {
        let mut doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\nmeta:Purpose: old\n- note\n\n## Notes\n\n## To-dos\n",
        );
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Purpose", "new");
        let text = doc.to_text();
        assert!(text.contains("meta:Purpose: new\n"), "got: {}", text);
        assert!(
            !text.contains("meta:Purpose: old\n"),
            "old should be gone: {}",
            text
        );
    }

    #[test]
    fn set_metadata_field_migrates_legacy_time_fields() {
        // Old-format file has bare "Scheduled: HH:MM" — set_metadata_field should rewrite it
        let mut doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\nScheduled: 09:00\n- note\n\n## Notes\n\n## To-dos\n",
        );
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Started", "09:05");
        let text = doc.to_text();
        // Legacy Scheduled line should be rewritten to meta: prefix
        assert!(
            text.contains("meta:Scheduled: 09:00\n"),
            "legacy migrated: {}",
            text
        );
        assert!(
            text.contains("meta:Started: 09:05\n"),
            "new field written: {}",
            text
        );
        assert!(
            !text.contains("Scheduled: 09:00\n") || text.contains("meta:Scheduled:"),
            "legacy gone: {}",
            text
        );
    }

    #[test]
    fn set_metadata_field_canonical_order_with_purpose() {
        // Purpose, Topic, Scheduled, Started, Ended — regardless of insertion order
        let mut doc =
            Document::from_text("# Day\n\n## Meetings\n\n### Standup\n\n## Notes\n\n## To-dos\n");
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Ended", "10:00");
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Started", "09:05");
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Scheduled", "09:00");
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Purpose", "sync");
        let text = doc.to_text();
        let purpose_pos = text.find("meta:Purpose:").unwrap();
        let sched_pos = text.find("meta:Scheduled:").unwrap();
        let started_pos = text.find("meta:Started:").unwrap();
        let ended_pos = text.find("meta:Ended:").unwrap();
        assert!(purpose_pos < sched_pos, "Purpose before Scheduled");
        assert!(sched_pos < started_pos, "Scheduled before Started");
        assert!(started_pos < ended_pos, "Started before Ended");
    }

    #[test]
    fn set_metadata_field_does_not_eat_note_content_again() {
        let mut doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\n- note one\n- note two\n\n## Notes\n\n## To-dos\n",
        );
        let heading = doc.meetings()[0].heading_line;
        set_metadata_field(&mut doc.lines, heading, "Started", "09:15");
        let text = doc.to_text();
        assert!(
            text.contains("- note one\n"),
            "note one should remain: {}",
            text
        );
        assert!(
            text.contains("- note two\n"),
            "note two should remain: {}",
            text
        );
    }

    #[test]
    fn expand_indent_markers_zero_markers() {
        assert_eq!(expand_indent_markers("- item"), "- item");
    }

    #[test]
    fn expand_indent_markers_one_marker() {
        assert_eq!(expand_indent_markers("->- item"), "  - item");
    }

    #[test]
    fn expand_indent_markers_two_markers() {
        assert_eq!(expand_indent_markers("->->- item"), "    - item");
    }

    #[test]
    fn expand_indent_markers_three_markers() {
        assert_eq!(expand_indent_markers("->->->- item"), "      - item");
    }

    #[test]
    fn expand_indent_markers_plain_text() {
        assert_eq!(expand_indent_markers("->plain"), "  plain");
    }

    #[test]
    fn expand_indent_markers_mid_line_preserved() {
        assert_eq!(expand_indent_markers("hello -> world"), "hello -> world");
    }

    #[test]
    fn format_entry_single_indent_marker_becomes_bullet() {
        assert_eq!(format_entry("->- item", None), vec!["  - item"]);
    }

    #[test]
    fn format_entry_double_indent_marker() {
        assert_eq!(format_entry("->->- item", None), vec!["    - item"]);
    }

    #[test]
    fn format_entry_indent_marker_plain_text() {
        assert_eq!(format_entry("->plain", None), vec!["  plain"]);
    }

    #[test]
    fn format_entry_multiline_indent_markers() {
        assert_eq!(
            format_entry("->- parent\n->->- child", None),
            vec!["  - parent", "    - child"]
        );
    }

    #[test]
    fn add_entry_to_section_stays_within_it() {
        let mut doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\n\n#### Updates\n\n## Notes\n\n## To-dos\n",
        );
        let heading_line = doc.lines.iter().position(|l| l == "#### Updates").unwrap();
        doc.add_block(
            &EntryTarget::Section {
                heading_line,
                level: 4,
            },
            &["- note".to_string()],
        );
        let text = doc.to_text();
        let section_pos = text.find("#### Updates").unwrap();
        let entry_pos = text.find("- note").unwrap();
        let notes_pos = text.find("## Notes").unwrap();
        assert!(
            entry_pos > section_pos,
            "entry should be after section heading: {}",
            text
        );
        assert!(
            entry_pos < notes_pos,
            "entry should be before ## Notes: {}",
            text
        );
    }

    #[test]
    fn add_entry_to_section_stops_at_peer_heading() {
        // A #### section ends before the next ####
        let mut doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\n\n#### Alice\n\n#### Bob\n\n## Notes\n\n## To-dos\n",
        );
        let alice_line = doc.lines.iter().position(|l| l == "#### Alice").unwrap();
        doc.add_block(
            &EntryTarget::Section {
                heading_line: alice_line,
                level: 4,
            },
            &["- alice note".to_string()],
        );
        let text = doc.to_text();
        let alice_pos = text.find("#### Alice").unwrap();
        let entry_pos = text.find("- alice note").unwrap();
        let bob_pos = text.find("#### Bob").unwrap();
        assert!(
            entry_pos > alice_pos,
            "entry should be after Alice: {}",
            text
        );
        assert!(entry_pos < bob_pos, "entry should be before Bob: {}", text);
    }

    #[test]
    fn add_section_heading_in_meeting_creates_h4_returns_line() {
        let mut doc = Document::new_for_date(chrono::NaiveDate::from_ymd_opt(2026, 6, 8).unwrap());
        let ord = doc.add_meeting("Standup");
        let heading_line = doc.add_section_heading(&EntryTarget::Meeting(ord), 4, "Updates");
        let text = doc.to_text();
        assert!(text.contains("#### Updates\n"), "got: {}", text);
        let standup_pos = text.find("### Standup").unwrap();
        let section_pos = text.find("#### Updates").unwrap();
        assert!(
            section_pos > standup_pos,
            "section should be after meeting heading"
        );
        assert_eq!(
            doc.lines[heading_line], "#### Updates",
            "heading_line should point to the heading"
        );
    }

    #[test]
    fn add_section_heading_nested_in_section_creates_h5() {
        let mut doc = Document::new_for_date(chrono::NaiveDate::from_ymd_opt(2026, 6, 8).unwrap());
        let ord = doc.add_meeting("Standup");
        let h4_line = doc.add_section_heading(&EntryTarget::Meeting(ord), 4, "Updates");
        let h5_line = doc.add_section_heading(
            &EntryTarget::Section {
                heading_line: h4_line,
                level: 4,
            },
            5,
            "Details",
        );
        let text = doc.to_text();
        assert!(text.contains("##### Details\n"), "got: {}", text);
        assert_eq!(doc.lines[h5_line], "##### Details");
        let updates_pos = text.find("#### Updates").unwrap();
        let details_pos = text.find("##### Details").unwrap();
        assert!(details_pos > updates_pos, "Details should be after Updates");
    }

    #[test]
    fn add_entry_to_nested_section_stays_within_it() {
        let mut doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\n\n#### Updates\n\n##### Details\n\n## Notes\n\n## To-dos\n",
        );
        let details_line = doc.lines.iter().position(|l| l == "##### Details").unwrap();
        doc.add_block(
            &EntryTarget::Section {
                heading_line: details_line,
                level: 5,
            },
            &["- detail".to_string()],
        );
        let text = doc.to_text();
        let details_pos = text.find("##### Details").unwrap();
        let entry_pos = text.find("- detail").unwrap();
        let notes_pos = text.find("## Notes").unwrap();
        assert!(entry_pos > details_pos, "entry should be after Details");
        assert!(entry_pos < notes_pos, "entry should be before ## Notes");
    }

    #[test]
    fn toggle_todo_at_line_unchecked_to_checked() {
        let mut doc = Document::from_text("# Day\n\n## To-dos\n\n- [ ] write tests\n");
        // line 4 is "- [ ] write tests"
        doc.toggle_todo_at_line(4).unwrap();
        assert_eq!(doc.lines[4], "- [x] write tests");
    }

    #[test]
    fn toggle_todo_at_line_checked_to_unchecked() {
        let mut doc = Document::from_text("# Day\n\n## To-dos\n\n- [x] done task\n");
        doc.toggle_todo_at_line(4).unwrap();
        assert_eq!(doc.lines[4], "- [ ] done task");
    }

    #[test]
    fn toggle_todo_at_line_uppercase_x_to_unchecked() {
        let mut doc = Document::from_text("# Day\n\n## To-dos\n\n- [X] done task\n");
        doc.toggle_todo_at_line(4).unwrap();
        assert_eq!(doc.lines[4], "- [ ] done task");
    }

    #[test]
    fn toggle_todo_at_line_non_todo_returns_err() {
        let mut doc = Document::from_text("# Day\n\n## Notes\n\n- just a bullet\n");
        let result = doc.toggle_todo_at_line(4);
        assert!(result.is_err(), "expected error for non-todo line");
    }

    #[test]
    fn toggle_todo_at_line_out_of_bounds_returns_err() {
        let mut doc = Document::from_text("# Day\n");
        let result = doc.toggle_todo_at_line(99);
        assert!(result.is_err());
    }

    #[test]
    fn meetings_with_scheduled_empty_when_no_meetings_section() {
        let doc = Document::from_text("# Day\n\n## Notes\n\n## To-dos\n");
        assert!(doc.meetings_with_scheduled().is_empty());
    }

    #[test]
    fn meetings_with_scheduled_excludes_meeting_without_scheduled() {
        let doc =
            Document::from_text("# Day\n\n## Meetings\n\n### Standup\n\n## Notes\n\n## To-dos\n");
        let result = doc.meetings_with_scheduled();
        assert!(result.is_empty());
    }

    #[test]
    fn meetings_with_scheduled_includes_meeting_with_scheduled() {
        let doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\nScheduled: 09:30\n\n## Notes\n\n## To-dos\n",
        );
        let result = doc.meetings_with_scheduled();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "09:30");
        assert_eq!(result[0].1, "Standup");
    }

    #[test]
    fn meetings_with_scheduled_extracts_only_scheduled_from_multiple_fields() {
        let doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\nScheduled: 09:00\nStarted: 09:05\nEnded: 09:45\n\n## Notes\n\n## To-dos\n",
        );
        let result = doc.meetings_with_scheduled();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "09:00");
        assert_eq!(result[0].1, "Standup");
    }

    #[test]
    fn meetings_with_scheduled_returns_only_meetings_with_scheduled() {
        let doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\nScheduled: 09:30\n\n### Review\n\n### Design\nScheduled: 14:00\n\n## Notes\n\n## To-dos\n",
        );
        let result = doc.meetings_with_scheduled();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "09:30");
        assert_eq!(result[0].1, "Standup");
        assert_eq!(result[1].0, "14:00");
        assert_eq!(result[1].1, "Design");
    }

    #[test]
    fn meetings_with_scheduled_sorted_by_time() {
        let doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Afternoon\nScheduled: 14:00\n\n### Morning\nScheduled: 09:00\n\n### Noon\nScheduled: 12:00\n\n## Notes\n\n## To-dos\n",
        );
        let result = doc.meetings_with_scheduled();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, "09:00");
        assert_eq!(result[1].0, "12:00");
        assert_eq!(result[2].0, "14:00");
    }

    #[test]
    fn meetings_with_scheduled_skips_malformed_scheduled() {
        let doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\nScheduled:\n\n## Notes\n\n## To-dos\n",
        );
        let result = doc.meetings_with_scheduled();
        assert!(result.is_empty());
    }

    #[test]
    fn meetings_with_scheduled_skips_heading_with_embedded_time() {
        let doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### 09:30 Standup\n\n## Notes\n\n## To-dos\n",
        );
        let result = doc.meetings_with_scheduled();
        assert!(result.is_empty());
    }

    #[test]
    fn add_meeting_summary_appends_at_end_of_meeting_section() {
        let mut doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\nmeta:Started: 09:00\n- note one\n- note two\n\n## Notes\n\n## To-dos\n",
        );
        doc.add_meeting_summary(0, "#### Meeting Summary\n**Key Decisions:** Ship it");
        let text = doc.to_text();
        let note_two_pos = text.find("- note two").unwrap();
        let summary_pos = text.find("#### Meeting Summary").unwrap();
        assert!(
            summary_pos > note_two_pos,
            "summary should be after notes: {}",
            text
        );
        // Must not bleed into Notes section
        let notes_pos = text.find("## Notes").unwrap();
        assert!(
            summary_pos < notes_pos,
            "summary must be before ## Notes: {}",
            text
        );
    }

    #[test]
    fn add_meeting_summary_replaces_existing_summary() {
        let mut doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\n- note\n\n#### Meeting Summary\n**Key Decisions:** old\n\n## Notes\n\n## To-dos\n",
        );
        doc.add_meeting_summary(0, "#### Meeting Summary\n**Key Decisions:** new");
        let text = doc.to_text();
        assert!(
            text.contains("**Key Decisions:** new"),
            "new summary missing: {}",
            text
        );
        assert!(
            !text.contains("**Key Decisions:** old"),
            "old summary should be gone: {}",
            text
        );
        // Only one #### Meeting Summary heading
        assert_eq!(
            text.matches("#### Meeting Summary").count(),
            1,
            "duplicate summary: {}",
            text
        );
    }

    #[test]
    fn add_meeting_summary_normalizes_missing_heading() {
        let mut doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\n- note\n\n## Notes\n\n## To-dos\n",
        );
        // AI response without the heading
        doc.add_meeting_summary(0, "**Key Decisions:** Ship it");
        let text = doc.to_text();
        assert!(
            text.contains("#### Meeting Summary"),
            "heading should be added: {}",
            text
        );
        assert!(
            text.contains("**Key Decisions:** Ship it"),
            "content should be present: {}",
            text
        );
    }

    #[test]
    fn add_meeting_summary_noop_for_invalid_ordinal() {
        let mut doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\n- note\n\n## Notes\n\n## To-dos\n",
        );
        let before = doc.to_text();
        doc.add_meeting_summary(99, "#### Meeting Summary\n**Key Decisions:** x");
        assert_eq!(doc.to_text(), before, "invalid ordinal should be noop");
    }

    #[test]
    fn add_meeting_summary_does_not_bleed_into_next_meeting() {
        let mut doc = Document::from_text(
            "# Day\n\n## Meetings\n\n### Standup\n- standup note\n\n### Review\n- review note\n\n## Notes\n\n## To-dos\n",
        );
        doc.add_meeting_summary(
            0,
            "#### Meeting Summary\n**Key Decisions:** standup decision",
        );
        let text = doc.to_text();
        let summary_pos = text.find("#### Meeting Summary").unwrap();
        let review_pos = text.find("### Review").unwrap();
        assert!(
            summary_pos < review_pos,
            "summary should not pass the Review meeting: {}",
            text
        );
        assert!(
            text.contains("- review note"),
            "review notes must be intact: {}",
            text
        );
    }
}
