use crate::model::day::SectionKind;

pub fn heading_line(lines: &[String], kind: SectionKind) -> Option<usize> {
    let target = match kind {
        SectionKind::Meetings => "## Meetings",
        SectionKind::Notes => "## Notes",
        SectionKind::Todos => "## To-dos",
    };
    lines.iter().position(|line| line == target)
}

pub fn section_end(lines: &[String], start: usize) -> usize {
    lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .position(|(_, line)| line.starts_with("## "))
        .map(|i| start + 1 + i)
        .unwrap_or(lines.len())
}

pub fn ensure_section(lines: &mut Vec<String>, kind: SectionKind) -> usize {
    if let Some(idx) = heading_line(lines, kind) {
        return idx;
    }

    if lines.last().map(|s| !s.is_empty()).unwrap_or(true) {
        lines.push(String::new());
    }

    let heading = match kind {
        SectionKind::Meetings => "## Meetings",
        SectionKind::Notes => "## Notes",
        SectionKind::Todos => "## To-dos",
    };
    lines.push(heading.to_string());
    lines.len() - 1
}

pub fn block_insert_index(lines: &[String], start_excl: usize, end_excl: usize) -> usize {
    for i in (start_excl + 1..end_excl).rev() {
        if !lines[i].trim().is_empty() {
            return i + 1;
        }
    }
    start_excl + 1
}

/// Number of leading `#` (1..=6) if the line is an ATX heading (`#` then a space).
pub fn heading_level(line: &str) -> Option<usize> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && line.chars().nth(hashes) == Some(' ') {
        Some(hashes)
    } else {
        None
    }
}

/// True if the line starts an ordered-list item: digits then `. ` or `) `.
pub fn is_ordered(line: &str) -> bool {
    let t = line.trim_start();
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return false;
    }
    let rest = &t[digits..];
    rest.starts_with(". ") || rest.starts_with(") ")
}

pub fn is_section_heading(line: &str) -> bool {
    matches!(line, "## Meetings" | "## Notes" | "## To-dos")
}

pub fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

pub fn is_quote(line: &str) -> bool {
    let t = line.trim_start();
    t == ">" || t.starts_with("> ")
}

pub fn is_bullet(line: &str) -> bool {
    let t = line.trim_start();
    (t.starts_with("- ") && !t.starts_with("- [")) || t.starts_with("* ") || t.starts_with("+ ")
}

/// `Some(false)` for `- [ ]`, `Some(true)` for `- [x]`/`- [X]`, else `None`.
pub fn todo_state(line: &str) -> Option<bool> {
    let t = line.trim_start();
    if t.starts_with("- [ ] ") {
        Some(false)
    } else if t.starts_with("- [x] ") || t.starts_with("- [X] ") {
        Some(true)
    } else {
        None
    }
}

/// Index after the last continuation line starting at `from`. A continuation
/// line is non-blank and indented by at least two spaces (or a tab).
/// Indented lines that look like new structural elements (bullet, todo,
/// ordered list, heading, quote, or code fence) are not continuations.
pub fn continuation_end(lines: &[String], from: usize) -> usize {
    let mut j = from;
    while j < lines.len() {
        let l = &lines[j];
        if l.trim().is_empty() {
            break;
        }
        let indent = l.len() - l.trim_start().len();
        if indent >= 2 || l.starts_with('\t') {
            if is_bullet(l)
                || todo_state(l).is_some()
                || is_ordered(l)
                || heading_level(l).is_some()
                || is_quote(l)
                || is_fence(l)
                || is_section_heading(l)
            {
                break;
            }
            j += 1;
        } else {
            break;
        }
    }
    j
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lines(text: &str) -> Vec<String> {
        text.lines().map(|s| s.to_string()).collect()
    }

    #[test]
    fn heading_line_finds_each_section() {
        let lines = make_lines("# 2026-06-04 (Thu)\n\n## Meetings\n\n## Notes\n\n## To-dos\n");
        assert_eq!(heading_line(&lines, SectionKind::Meetings), Some(2));
        assert_eq!(heading_line(&lines, SectionKind::Notes), Some(4));
        assert_eq!(heading_line(&lines, SectionKind::Todos), Some(6));
    }

    #[test]
    fn heading_line_returns_none_for_missing() {
        let lines = make_lines("# 2026-06-04 (Thu)\n\n## Meetings\n\n## Notes\n");
        assert_eq!(heading_line(&lines, SectionKind::Todos), None);
    }

    #[test]
    fn section_end_finds_next_heading() {
        let lines = make_lines("## Meetings\n- foo\n## Notes\n");
        assert_eq!(section_end(&lines, 0), 2);
    }

    #[test]
    fn section_end_returns_len_when_no_next_heading() {
        let lines = make_lines("## Meetings\n- foo\n");
        assert_eq!(section_end(&lines, 0), 2);
    }

    #[test]
    fn ensure_section_returns_existing_index() {
        let mut lines = make_lines("## Meetings\n\n## Notes\n");
        let idx = ensure_section(&mut lines, SectionKind::Notes);
        assert_eq!(idx, 2);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn ensure_section_missing_appends_heading() {
        let mut lines = make_lines("## Meetings\n");
        let idx = ensure_section(&mut lines, SectionKind::Notes);
        assert_eq!(idx, 2);
        assert_eq!(lines, vec!["## Meetings", "", "## Notes"]);
    }

    #[test]
    fn ensure_section_missing_when_already_blank_before() {
        let mut lines = make_lines("## Meetings\n\n");
        let idx = ensure_section(&mut lines, SectionKind::Notes);
        assert_eq!(idx, 2);
        assert_eq!(lines, vec!["## Meetings", "", "## Notes"]);
    }

    #[test]
    fn ensure_section_on_empty_vec() {
        let mut lines: Vec<String> = Vec::new();
        let idx = ensure_section(&mut lines, SectionKind::Todos);
        assert_eq!(idx, 1);
        assert_eq!(lines, vec!["", "## To-dos"]);
    }

    #[test]
    fn block_insert_index_empty_block() {
        let lines = make_lines("## Meetings\n\n## Notes\n");
        let start = heading_line(&lines, SectionKind::Meetings).unwrap();
        let end = section_end(&lines, start);
        assert_eq!(block_insert_index(&lines, start, end), start + 1);
    }

    #[test]
    fn block_insert_index_populated_block() {
        let lines = make_lines("## Meetings\n\n- foo\n- bar\n\n## Notes\n");
        let start = heading_line(&lines, SectionKind::Meetings).unwrap();
        let end = section_end(&lines, start);
        assert_eq!(block_insert_index(&lines, start, end), 4);
    }

    #[test]
    fn is_bullet_recognizes_indented() {
        assert!(is_bullet("  - item"));
        assert!(is_bullet("    * item"));
        assert!(is_bullet("\t+ item"));
        assert!(!is_bullet("  - [ ] todo"));
        assert!(!is_bullet("plain"));
    }

    #[test]
    fn todo_state_recognizes_indented() {
        assert_eq!(todo_state("  - [ ] task"), Some(false));
        assert_eq!(todo_state("    - [x] done"), Some(true));
        assert_eq!(todo_state("  plain"), None);
    }

    #[test]
    fn is_ordered_recognizes_indented() {
        assert!(is_ordered("  1. first"));
        assert!(is_ordered("    2) second"));
        assert!(!is_ordered("  plain"));
    }

    #[test]
    fn continuation_end_stops_at_indented_bullet() {
        let lines = make_lines("- parent\n  - child\n");
        assert_eq!(continuation_end(&lines, 1), 1);
    }

    #[test]
    fn continuation_end_stops_at_indented_todo() {
        let lines = make_lines("- parent\n  - [ ] child\n");
        assert_eq!(continuation_end(&lines, 1), 1);
    }

    #[test]
    fn continuation_end_stops_at_indented_ordered() {
        let lines = make_lines("- parent\n  1. child\n");
        assert_eq!(continuation_end(&lines, 1), 1);
    }

    #[test]
    fn continuation_end_includes_plain_indented_text() {
        let lines = make_lines("- parent\n  cont\n  more\n");
        assert_eq!(continuation_end(&lines, 1), 3);
    }

    #[test]
    fn continuation_end_mixed_parent_then_sub_bullet() {
        let lines = make_lines("- parent\n  cont\n  - child\n");
        assert_eq!(continuation_end(&lines, 1), 2);
    }
}
