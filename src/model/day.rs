use chrono::NaiveDate;

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
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SelectableKind {
    Entry,
    Todo { done: bool },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Selectable {
    pub line: usize,
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

pub struct Document {
    lines: Vec<String>,
}

impl Document {
    pub fn from_text(text: &str) -> Document {
        let mut lines: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
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
            String::new(),
            "## Meetings".to_string(),
            String::new(),
            "## Notes".to_string(),
            String::new(),
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
    fn new_for_date_template() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        let doc = Document::new_for_date(date);
        let expected = "# 2026-06-04 (Thu)\n\n## Meetings\n\n## Notes\n\n## To-dos\n";
        assert_eq!(doc.to_text(), expected);
    }
}
