use crate::app::state::{AppState, Focus};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

pub fn render(frame: &mut ratatui::Frame, app: &AppState, area: Rect) {
    let selected_range: Option<std::ops::Range<usize>> = if app.focus == Focus::Navigate {
        app.selectables.get(app.selected).map(|s| s.lines.clone())
    } else {
        None
    };

    let mut in_code = false;
    let text_lines: Vec<Line> = app
        .doc
        .lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let is_selected = selected_range
                .as_ref()
                .map(|r| r.contains(&i))
                .unwrap_or(false);
            let highlight = if is_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };

            let fence = line.trim_start().starts_with("```");
            if in_code || fence {
                if fence {
                    in_code = !in_code;
                }
                return Line::from(Span::styled(
                    line.as_str(),
                    Style::default().fg(Color::DarkGray),
                ))
                .style(highlight);
            }

            if let Some(rest) = line.strip_prefix("# ") {
                Line::from(vec![Span::styled(
                    format!("# {}", rest),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                )])
                .style(highlight)
            } else if let Some(rest) = line.strip_prefix("## ") {
                Line::from(vec![Span::styled(
                    format!("## {}", rest),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                )])
                .style(highlight)
            } else if let Some(rest) = line.strip_prefix("### ") {
                Line::from(vec![Span::styled(
                    format!("### {}", rest),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )])
                .style(highlight)
            } else if let Some(rest) = line.strip_prefix("- [ ] ") {
                Line::from(vec![Span::raw("☐ "), Span::raw(rest)]).style(highlight)
            } else if let Some(rest) = line
                .strip_prefix("- [x] ")
                .or_else(|| line.strip_prefix("- [X] "))
            {
                Line::from(vec![
                    Span::styled("☑ ", Style::default().fg(Color::Green)),
                    Span::styled(
                        rest,
                        Style::default().fg(Color::Green).add_modifier(Modifier::CROSSED_OUT),
                    ),
                ])
                .style(highlight)
            } else if let Some(rest) = line
                .strip_prefix("> ")
                .or_else(|| if line == ">" { Some("") } else { None })
            {
                Line::from(vec![
                    Span::styled("│ ", Style::default().fg(Color::Magenta).add_modifier(Modifier::ITALIC)),
                    Span::styled(rest, Style::default().add_modifier(Modifier::ITALIC)),
                ])
                .style(highlight)
            } else if let Some(rest) = line
                .strip_prefix("- ")
                .or_else(|| line.strip_prefix("* "))
                .or_else(|| line.strip_prefix("+ "))
            {
                Line::from(vec![Span::raw("• "), Span::raw(rest)]).style(highlight)
            } else if crate::model::parser::is_ordered(line) {
                Line::from(Span::raw(line.as_str())).style(highlight)
            } else {
                Line::from(line.as_str()).style(highlight)
            }
        })
        .collect();

    let scroll_offset = if let Some(r) = selected_range {
        let visible_height = area.height as usize;
        r.end.saturating_sub(visible_height)
    } else {
        0
    };

    let paragraph = Paragraph::new(Text::from(text_lines)).scroll((scroll_offset as u16, 0));
    frame.render_widget(paragraph, area);
}
