use anyhow::{Context, Result};
use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

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

fn read_key() -> Result<Option<ratatui::crossterm::event::KeyEvent>> {
    if !ratatui::crossterm::event::poll(std::time::Duration::from_millis(100))? {
        return Ok(None);
    }
    match ratatui::crossterm::event::read()? {
        Event::Key(key) if key.kind == KeyEventKind::Press => Ok(Some(key)),
        _ => Ok(None),
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
                    return Err(anyhow::anyhow!("--notes-dir requires a value"));
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
                return Err(anyhow::anyhow!("Unknown flag: {}", arg));
            }
        }
    }

    // Load config and open today's note
    let (config, notes_dir) = kua_tin::config::load(cli_notes_dir).context("Config error")?;

    let mut app = kua_tin::app::state::AppState::open_day(
        notes_dir,
        config,
        chrono::Local::now().date_naive(),
    )
    .context("Failed to open day")?;

    // Initialize terminal
    let mut terminal = ratatui::init();
    let _guard = TerminalGuard;

    use kua_tin::app::state::{Focus, Overlay};

    // Main event loop
    loop {
        terminal.draw(|frame| {
            kua_tin::ui::render(frame, &app);
        })?;

        if let Some(key) = read_key()? {
            // Ctrl-C always quits
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                break Ok(());
            }

            // Overlay handling
            match app.overlay {
                Overlay::Calendar => {
                    match key.code {
                        KeyCode::Left => {
                            if let Some(cal) = app.calendar.as_mut() {
                                kua_tin::ui::calendar::move_selection(cal, -1, 0);
                            }
                        }
                        KeyCode::Right => {
                            if let Some(cal) = app.calendar.as_mut() {
                                kua_tin::ui::calendar::move_selection(cal, 1, 0);
                            }
                        }
                        KeyCode::Up => {
                            if let Some(cal) = app.calendar.as_mut() {
                                kua_tin::ui::calendar::move_selection(cal, 0, -1);
                            }
                        }
                        KeyCode::Down => {
                            if let Some(cal) = app.calendar.as_mut() {
                                kua_tin::ui::calendar::move_selection(cal, 0, 1);
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(cal) = app.calendar.take() {
                                kua_tin::app::actions::go_to_date(&mut app, cal.selected)?;
                                app.status.clear();
                                app.overlay = Overlay::None;
                            }
                        }
                        KeyCode::Esc => {
                            app.overlay = Overlay::None;
                            app.calendar = None;
                        }
                        _ => {}
                    }
                    continue;
                }
                Overlay::Help => {
                    if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
                        app.overlay = Overlay::None;
                    }
                    continue;
                }
                Overlay::None => {}
            }

            // Global keys
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('t') {
                kua_tin::app::actions::go_today(&mut app)?;
                app.status.clear();
                continue;
            }
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('g') {
                app.pending_delete = false;
                app.calendar = Some(kua_tin::ui::calendar::CalendarState::new(app.date));
                app.overlay = Overlay::Calendar;
                continue;
            }

            // Esc
            if key.code == KeyCode::Esc {
                match app.focus {
                    Focus::Capture => {
                        if app.editing.is_some() {
                            app.editing = None;
                            app.input.clear();
                        } else {
                            app.focus = Focus::Navigate;
                        }
                    }
                    Focus::Navigate => {
                        app.pending_delete = false;
                        app.focus = Focus::Capture;
                    }
                }
                continue;
            }

            // [ and ] navigation
            let can_navigate = match app.focus {
                Focus::Navigate => true,
                Focus::Capture => app.input.is_empty(),
            };
            if can_navigate {
                match key.code {
                    KeyCode::Char('[') => {
                        kua_tin::app::actions::go_prev_day(&mut app)?;
                        continue;
                    }
                    KeyCode::Char(']') => {
                        kua_tin::app::actions::go_next_day(&mut app)?;
                        continue;
                    }
                    _ => {}
                }
            }

            // Mode-specific keys
            match app.focus {
                Focus::Capture => {
                    match key.code {
                        KeyCode::Char(c)
                            if !key.modifiers.contains(KeyModifiers::CONTROL)
                                && !c.is_control() =>
                        {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        KeyCode::Enter => {
                            if key.modifiers.contains(KeyModifiers::ALT) {
                                app.input.push('\n');
                            } else if app.editing.is_some() {
                                kua_tin::app::actions::commit_edit(&mut app)?;
                            } else if !app.input.is_empty() {
                                // Fallback for terminals that don't send ALT with Enter
                                // (e.g. macOS Terminal.app / iTerm2 without "Use Option as Meta").
                                // When input is non-empty, plain Enter inserts a newline.
                                app.input.push('\n');
                            } else {
                                let cmd = kua_tin::app::command::parse(&app.input);
                                kua_tin::app::actions::dispatch(&mut app, cmd)?;
                                if app.overlay != Overlay::None {
                                    app.pending_delete = false;
                                }
                                app.input.clear();
                            }
                        }
                        KeyCode::Up | KeyCode::Down => {
                            // ignored in capture mode
                        }
                        _ => {}
                    }
                }
                Focus::Navigate => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        // ignore Ctrl combos in navigate mode
                    } else if app.pending_delete {
                        match key.code {
                            KeyCode::Char('d') => {
                                if let Err(e) = kua_tin::app::actions::delete_selected(&mut app) {
                                    app.status = e.to_string();
                                }
                                app.pending_delete = false;
                                continue;
                            }
                            _ => {
                                app.pending_delete = false;
                                // fall through to normal handling
                            }
                        }
                    }

                    match key.code {
                        KeyCode::Char('j') | KeyCode::Down => {
                            kua_tin::app::actions::select_next(&mut app);
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            kua_tin::app::actions::select_prev(&mut app);
                        }
                        KeyCode::Char('g') => {
                            kua_tin::app::actions::select_first(&mut app);
                        }
                        KeyCode::Char('G') => {
                            kua_tin::app::actions::select_last(&mut app);
                        }
                        KeyCode::Char(' ') | KeyCode::Char('x') => {
                            kua_tin::app::actions::toggle_selected(&mut app);
                        }
                        KeyCode::Char('e') => {
                            kua_tin::app::actions::begin_edit_selected(&mut app);
                        }
                        KeyCode::Char('d') => {
                            app.pending_delete = true;
                        }
                        KeyCode::Enter => {
                            kua_tin::app::actions::resume_selected_meeting(&mut app);
                        }
                        KeyCode::Char('?') => {
                            app.pending_delete = false;
                            app.overlay = Overlay::Help;
                        }
                        KeyCode::Char('i') => {
                            app.pending_delete = false;
                            app.focus = Focus::Capture;
                        }
                        _ => {}
                    }
                }
            }
        }

        if app.should_quit {
            break Ok(());
        }
    }
}
