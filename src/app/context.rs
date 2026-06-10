use crate::app::state::Context;

/// Find the line index of the nearest "## " heading at or before `cursor_line`.
fn enclosing_l2_heading(lines: &[String], cursor_line: usize) -> Option<usize> {
    (0..=cursor_line).rev().find(|&i| lines[i].starts_with("## "))
}

/// Find the index of the last "### " heading in `lines[start..=end]`.
/// Stops scanning if a "## " heading is encountered (crossed into another section).
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

/// Find the index and level of the last "####"+ heading in `lines[(l3_line+1)..=end]`.
/// Returns `None` if no such heading exists, or if a "### " heading resets the search.
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

/// Count the number of "### " headings in `lines[start..=end]`.
/// Used to compute the zero-based ordinal for Meeting/NoteBlock context.
fn count_l3_headings(lines: &[String], start: usize, end: usize) -> usize {
    lines[start..=end]
        .iter()
        .filter(|l| l.starts_with("### "))
        .count()
}

/// Derive the editing context from a cursor position in the document.
/// Used to update `state.context` automatically as the vim cursor moves.
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

/// Find the line index of the first line in `lines` that exactly equals `heading`.
/// Returns `None` if not found.
fn find_heading(lines: &[String], heading: &str) -> Option<usize> {
    lines.iter().position(|l| l == heading)
}

/// Find the line index of the Nth (0-based) `### ` heading that appears after
/// the given `##` section heading. Stops scanning at the next `## ` heading.
/// Returns `None` if the section or Nth heading is not found.
fn find_nth_l3_heading_in_section(
    lines: &[String],
    section: &str,
    n: usize,
) -> Option<usize> {
    let start = find_heading(lines, section)?;
    let mut count = 0usize;
    for (offset, line) in lines[start + 1..].iter().enumerate() {
        if line.starts_with("## ") {
            break; // entered a different section
        }
        if line.starts_with("### ") {
            if count == n {
                return Some(start + 1 + offset);
            }
            count += 1;
        }
    }
    None
}

/// Returns the line index in `lines` of the heading that corresponds to `context`.
/// Used to compute the Capture-mode scroll anchor when entering Capture from VimNormal.
/// Falls back to `0` (top of document) if the heading cannot be located.
pub fn context_heading_line(lines: &[String], context: &Context) -> usize {
    match context {
        Context::Section { heading_line, .. } => *heading_line,
        Context::Notes => find_heading(lines, "## Notes").unwrap_or(0),
        Context::Todos => find_heading(lines, "## To-dos").unwrap_or(0),
        Context::Meeting(n) => {
            find_nth_l3_heading_in_section(lines, "## Meetings", *n).unwrap_or(0)
        }
        Context::NoteBlock(n) => {
            find_nth_l3_heading_in_section(lines, "## Notes", *n).unwrap_or(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(|l| l.to_string()).collect()
    }

    // --- helper unit tests ---

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
        // range 1..=3 (after the ## heading)
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
        // l3_line=2 (the "### Second" line), cursor at 3
        assert_eq!(last_l4plus_heading(&doc, 2, 3), None);
    }

    #[test]
    fn count_l3_headings_counts_correctly() {
        let doc = lines("## Meetings\n### First\ncontent\n### Second\ncontent");
        assert_eq!(count_l3_headings(&doc, 1, 4), 2);
    }

    // --- context_at_line integration tests (moved from state.rs) ---

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
    fn context_heading_line_notes_returns_notes_heading() {
        let doc = lines("## Meetings\n\n## Notes\nstuff\n\n## To-dos\n");
        assert_eq!(context_heading_line(&doc, &Context::Notes), 2);
    }

    #[test]
    fn context_heading_line_todos_returns_todos_heading() {
        let doc = lines("## Meetings\n\n## Notes\n\n## To-dos\n");
        assert_eq!(context_heading_line(&doc, &Context::Todos), 4);
    }

    #[test]
    fn context_heading_line_meeting_zero_returns_first_meeting() {
        let doc = lines("## Meetings\n### Standup\ncontent\n### Planning\ncontent\n");
        assert_eq!(context_heading_line(&doc, &Context::Meeting(0)), 1);
    }

    #[test]
    fn context_heading_line_meeting_one_returns_second_meeting() {
        let doc = lines("## Meetings\n### Standup\ncontent\n### Planning\ncontent\n");
        assert_eq!(context_heading_line(&doc, &Context::Meeting(1)), 3);
    }

    #[test]
    fn context_heading_line_note_block_zero_returns_first_note() {
        let doc = lines("## Notes\n### My Note\ncontent\n");
        assert_eq!(context_heading_line(&doc, &Context::NoteBlock(0)), 1);
    }

    #[test]
    fn context_heading_line_section_returns_heading_line_directly() {
        let doc = lines("## Meetings\n### Standup\n#### Phase 1\ncontent\n");
        assert_eq!(
            context_heading_line(&doc, &Context::Section { heading_line: 2, level: 4 }),
            2,
        );
    }

    #[test]
    fn context_heading_line_missing_heading_returns_zero() {
        let doc = lines("## Meetings\n");
        assert_eq!(context_heading_line(&doc, &Context::Notes), 0);
    }

    #[test]
    fn cursor_on_empty_lines_vec() {
        assert_eq!(context_at_line(&[], 0), Context::Notes);
    }
}
