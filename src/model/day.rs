use chrono::NaiveDate;
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SectionKind {
    Meetings,
    Notes,
    Todos,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EntryTarget {
    Notes,
    Meeting(usize),
    NoteBlock(usize),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SelectableKind {
    Bullet,
    Todo { done: bool },
    MeetingHeading { ordinal: usize },
    NoteHeading { ordinal: usize },
    MarkdownHeading,
    Quote,
    Numbered,
    CodeBlock,
    Raw,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Selectable {
    pub lines: Range<usize>,
    pub kind: SelectableKind,
    pub text: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Meeting {
    pub ordinal: usize,
    pub heading_line: usize,
    pub time: String,
    pub name: String,
}

#[derive(Debug)]
pub struct Document {
    pub(crate) lines: Vec<String>,
}

impl Document {
    pub fn from_text(text: &str) -> Document {
        let mut lines: Vec<String> = text
            .split('\n')
            .map(|s| s.strip_suffix('\r').unwrap_or(s).to_string())
            .collect();
        if lines.last().map(|s| s.is_empty()).unwrap_or(false) {
            lines.pop();
        }
        Document { lines }
    }

    pub fn to_text(&self) -> String {
        if self.lines.is_empty() {
            return "\n".to_string();
        }
        let mut result = self.lines.join("\n");
        result.push('\n');
        result
    }

    pub fn new_for_date(date: NaiveDate) -> Document {
        let title = date.format("%Y-%m-%d (%a)").to_string();
        let lines = vec![
            format!("# {}", title),
            "".to_string(),
            "## Meetings".to_string(),
            "".to_string(),
            "## Notes".to_string(),
            "".to_string(),
            "## To-dos".to_string(),
        ];
        Document { lines }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_text_to_text_roundtrip() {
        let input = "a\nb\n";
        let doc = Document::from_text(input);
        assert_eq!(doc.to_text(), input);
    }

    #[test]
    fn from_text_empty_normalizes_to_single_trailing_newline() {
        assert_eq!(Document::from_text("").to_text(), "\n");
    }

    #[test]
    fn from_text_no_trailing_newline_normalizes() {
        assert_eq!(Document::from_text("a").to_text(), "a\n");
    }

    #[test]
    fn from_text_with_trailing_newline_preserved() {
        assert_eq!(Document::from_text("a\n").to_text(), "a\n");
    }

    #[test]
    fn from_text_multiple_lines_with_trailing_newline_preserved() {
        assert_eq!(Document::from_text("a\nb\n").to_text(), "a\nb\n");
    }

    #[test]
    fn from_text_crlf_strips_carriage_return() {
        let doc = Document::from_text("a\r\nb\r\n");
        assert_eq!(doc.to_text(), "a\nb\n");
    }

    #[test]
    fn new_for_date_template() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        let doc = Document::new_for_date(date);
        let expected = "# 2026-06-04 (Thu)\n\n## Meetings\n\n## Notes\n\n## To-dos\n";
        assert_eq!(doc.to_text(), expected);
    }
}
