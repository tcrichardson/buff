use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

pub fn render(frame: &mut ratatui::Frame, area: Rect) {
    let popup_width = 60;
    let popup_height = 24;
    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(popup_width)) / 2,
        y: area.y + (area.height.saturating_sub(popup_height)) / 2,
        width: popup_width.min(area.width),
        height: popup_height.min(area.height),
    };

    frame.render_widget(Clear, popup_area);

    let help_text = r#"Capture mode:
  type to enter notes, Enter to submit, Esc to navigate
  Tab        insert indent (->)
  Ctrl+.     prepend indent at line start

Commands:
  /meeting "Name"  start meeting context
  /note "Name"     start note context
  /note            switch to Notes context
  /todo text       add todo
  /start           record meeting start (current time)
  /end             record meeting end (current time)
  /scheduled HH:MM  record scheduled start time
  /leave           exit meeting context
  /goto YYYY-MM-DD  jump to date
  /today, Ctrl-T   jump to today
  /ask message     ask the local LLM (streams to chat)
  /clear           clear the chat conversation

Chat panel:
  Ctrl-L           show/hide chat panel
  Tab              focus chat (then j/k to scroll)

Navigation:
  [ ]        prev/next day
  j/k        move up/down
  g/G        first/last
  Space/x    toggle
  e          edit
  d d        delete
  i/Esc      capture mode
  ?          help
  Ctrl-C     quit

Right panel:
  Tab        focus right panel
  j/k or ↑/↓  navigate panel todos
  Space/x    toggle selected todo
  Esc        return to document"#;

    let block = Block::default().title("Help").borders(Borders::ALL);
    let paragraph = Paragraph::new(help_text).block(block);
    frame.render_widget(paragraph, popup_area);
}
