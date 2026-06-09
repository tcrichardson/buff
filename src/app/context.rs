use crate::app::state::Context;

fn enclosing_l2_heading(lines: &[String], cursor_line: usize) -> Option<usize> {
    (0..=cursor_line).rev().find(|&i| lines[i].starts_with("## "))
}

fn last_l3_heading(lines: &[String], start: usize, end: usize) -> Option<usize> {
    let mut result = None;
    for i in start..=end {
        if lines[i].starts_with("## ") {
            break;
        }
        if lines[i].starts_with("### ") {
            result = Some(i);
        }
    }
    result
}

fn last_l4plus_heading(lines: &[String], l3_line: usize, end: usize) -> Option<(usize, u8)> {
    let mut result = None;
    for i in (l3_line + 1)..=end {
        let line = &lines[i];
        if line.starts_with("## ") || line.starts_with("### ") {
            break;
        }
        if line.starts_with("#### ")
            || line.starts_with("##### ")
            || line.starts_with("###### ")
        {
            let level = line.chars().take_while(|&c| c == '#').count() as u8;
            result = Some((i, level));
        }
    }
    result
}

fn count_l3_headings(lines: &[String], start: usize, end: usize) -> usize {
    lines[start..=end]
        .iter()
        .filter(|l| l.starts_with("### "))
        .count()
}

pub fn context_at_line(lines: &[String], cursor_line: usize) -> Context {
    if lines.is_empty() || cursor_line >= lines.len() {
        return Context::Notes;
    }

    let boundary = match enclosing_l2_heading(lines, cursor_line) {
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

    let l3 = match last_l3_heading(lines, boundary + 1, cursor_line) {
        Some(l) => l,
        None => return Context::Notes,
    };

    if let Some((l4, level)) = last_l4plus_heading(lines, l3, cursor_line) {
        return Context::Section { heading_line: l4, level };
    }

    let ordinal = count_l3_headings(lines, boundary + 1, l3).saturating_sub(1);
    if in_meetings {
        Context::Meeting(ordinal)
    } else {
        Context::NoteBlock(ordinal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(|l| l.to_string()).collect()
    }

    #[test]
    fn enclosing_l2_finds_nearest_above() {
        let doc = lines("## Meetings\n### Standup\nsome line");
        assert_eq!(enclosing_l2_heading(&doc, 2), Some(0));
    }

    #[test]
    fn enclosing_l2_returns_none_when_absent() {
        let doc = lines("### Standup\nsome line");
        assert_eq!(enclosing_l2_heading(&doc, 1), None);
    }

    #[test]
    fn last_l3_finds_last_in_range() {
        let doc = lines("## Meetings\n### First\n### Second\ncontent");
        assert_eq!(last_l3_heading(&doc, 1, 3), Some(2));
    }

    #[test]
    fn last_l3_stops_at_l2_boundary() {
        let doc = lines("## Meetings\n### First\n## Notes\n### Other");
        assert_eq!(last_l3_heading(&doc, 1, 3), Some(1));
    }

    #[test]
    fn last_l4plus_finds_heading_after_l3() {
        let doc = lines("### Meeting\n#### Phase 1\ncontent");
        assert_eq!(last_l4plus_heading(&doc, 0, 2), Some((1, 4)));
    }

    #[test]
    fn last_l4plus_returns_none_when_new_l3_resets() {
        let doc = lines("### First\n#### Phase\n### Second\ncontent");
        assert_eq!(last_l4plus_heading(&doc, 2, 3), None);
    }

    #[test]
    fn count_l3_headings_counts_correctly() {
        let doc = lines("## Meetings\n### First\ncontent\n### Second\ncontent");
        assert_eq!(count_l3_headings(&doc, 1, 4), 2);
    }

    #[test]
    fn cursor_above_all_sections_is_notes() {
        let doc = lines("some preamble\n## Meetings");
        assert_eq!(context_at_line(&doc, 0), Context::Notes);
    }

    #[test]
    fn cursor_in_meetings_no_heading_is_notes() {
        let doc = lines("## Meetings\nno meeting heading yet");
        assert_eq!(context_at_line(&doc, 1), Context::Notes);
    }

    #[test]
    fn cursor_on_meeting_heading_is_meeting_0() {
        let doc = lines("## Meetings\n### Standup");
        assert_eq!(context_at_line(&doc, 1), Context::Meeting(0));
    }

    #[test]
    fn cursor_in_second_meeting_is_meeting_1() {
        let doc = lines("## Meetings\n### First\nstuff\n### Second\ncontent");
        assert_eq!(context_at_line(&doc, 4), Context::Meeting(1));
    }

    #[test]
    fn cursor_in_section_under_meeting() {
        let doc = lines("## Meetings\n### Standup\n#### Phase 1\ncontent");
        assert_eq!(
            context_at_line(&doc, 3),
            Context::Section { heading_line: 2, level: 4 }
        );
    }

    #[test]
    fn cursor_in_todos_section() {
        let doc = lines("## To-dos\n- [ ] something");
        assert_eq!(context_at_line(&doc, 1), Context::Todos);
    }

    #[test]
    fn cursor_in_note_block() {
        let doc = lines("## Notes\n### My Note\ncontent");
        assert_eq!(context_at_line(&doc, 2), Context::NoteBlock(0));
    }

    #[test]
    fn cursor_on_empty_lines_vec() {
        assert_eq!(context_at_line(&[], 0), Context::Notes);
    }
}
