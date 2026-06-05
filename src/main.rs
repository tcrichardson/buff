use std::io;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    // Parse CLI args manually
    let mut args = std::env::args().skip(1);
    let mut cli_notes_dir = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--notes-dir" => {
                cli_notes_dir = args.next();
            }
            "--help" => {
                println!("Usage: kua-tin [--notes-dir <path>]");
                return Ok(());
            }
            "--version" => {
                println!("kua-tin 0.1.0");
                return Ok(());
            }
            _ => {
                eprintln!("Unknown flag: {}", arg);
                std::process::exit(1);
            }
        }
    }

    // Load config and open today's note
    let (config, notes_dir) = match kua_tin::config::load(cli_notes_dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Config error: {}", e);
            std::process::exit(1);
        }
    };

    let app = match kua_tin::app::state::AppState::open_day(notes_dir, config, chrono::Local::now().date_naive()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to open day: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize terminal
    let mut terminal = ratatui::init();

    // Main event loop
    let result = loop {
        terminal.draw(|frame| {
            kua_tin::ui::render(frame, &app);
        })?;

        if ratatui::crossterm::event::poll(std::time::Duration::from_millis(100))? {
            if let ratatui::crossterm::event::Event::Key(key) = ratatui::crossterm::event::read()? {
                if key.kind == ratatui::crossterm::event::KeyEventKind::Press {
                    // For now, just handle Ctrl-C to quit
                    // Full key routing will be in Task 16
                    if key.code == ratatui::crossterm::event::KeyCode::Char('c')
                        && key.modifiers.contains(ratatui::crossterm::event::KeyModifiers::CONTROL)
                    {
                        break Ok(());
                    }
                }
            }
        }

        if app.should_quit {
            break Ok(());
        }
    };

    ratatui::restore();
    result
}
