use anyhow::{Context, Result};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

fn run() -> Result<()> {
    // Parse CLI args manually
    let mut args = std::env::args().skip(1);
    let mut cli_notes_dir = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--notes-dir" => match args.next() {
                Some(v) => cli_notes_dir = Some(v),
                None => {
                    eprintln!("Error: --notes-dir requires a value");
                    std::process::exit(1);
                }
            },
            "--help" => {
                println!("Usage: kua-tin [--notes-dir <path>]");
                return Ok(());
            }
            "--version" => {
                println!("kua-tin {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => {
                eprintln!("Unknown flag: {}", arg);
                std::process::exit(1);
            }
        }
    }

    // Load config and open today's note
    let (config, notes_dir) =
        kua_tin::config::load(cli_notes_dir).context("Config error")?;

    let app = kua_tin::app::state::AppState::open_day(
        notes_dir,
        config,
        chrono::Local::now().date_naive(),
    )
    .context("Failed to open day")?;

    // Initialize terminal
    let mut terminal = ratatui::init();
    let _guard = TerminalGuard;

    // Main event loop
    loop {
        terminal.draw(|frame| {
            kua_tin::ui::render(frame, &app);
        })?;

        if ratatui::crossterm::event::poll(std::time::Duration::from_millis(100))? {
            match ratatui::crossterm::event::read()? {
                ratatui::crossterm::event::Event::Key(key)
                    if key.kind == ratatui::crossterm::event::KeyEventKind::Press
                        && key.code == ratatui::crossterm::event::KeyCode::Char('c')
                        && key.modifiers
                            .contains(ratatui::crossterm::event::KeyModifiers::CONTROL) =>
                {
                    break Ok(());
                }
                _ => {}
            }
        }

        if app.should_quit {
            break Ok(());
        }
    }
}
