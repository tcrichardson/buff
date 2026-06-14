use anyhow::{Context, Result};
use ratatui::crossterm::event::{Event, KeyEventKind};
use ratatui::crossterm::{execute, cursor::SetCursorStyle};
use buff::app::state::Focus;

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

struct CliArgs {
    notes_dir: Option<String>,
}

fn parse_cli_args() -> Result<Option<CliArgs>> {
    let mut args = std::env::args().skip(1);
    let mut notes_dir = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--notes-dir" => match args.next() {
                Some(v) => notes_dir = Some(v),
                None => {
                    return Err(anyhow::anyhow!("--notes-dir requires a value"));
                }
            },
            "--help" => {
                println!("Usage: buff [--notes-dir <path>]");
                return Ok(None);
            }
            "--version" => {
                println!("buff {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown flag: {}", arg));
            }
        }
    }
    Ok(Some(CliArgs { notes_dir }))
}

fn set_cursor_style(focus: &Focus) -> Result<()> {
    let style = match focus {
        Focus::VimNormal => SetCursorStyle::SteadyBlock,
        Focus::VimInsert => SetCursorStyle::SteadyBar,
        _                => SetCursorStyle::DefaultUserShape,
    };
    execute!(std::io::stdout(), style)?;
    Ok(())
}

fn drain_llm_events(
    app: &mut buff::app::state::AppState,
    rx: &std::sync::mpsc::Receiver<buff::app::llm::LlmEvent>,
) {
    while let Ok(event) = rx.try_recv() {
        app.handle_llm_event(event);
    }
}

fn process_key(app: &mut buff::app::state::AppState) -> Result<buff::app::input::EventOutcome> {
    let Some(key) = read_key()? else {
        return Ok(buff::app::input::EventOutcome::Continue);
    };
    let Some(action) = buff::app::input::key_to_action(app, key) else {
        return Ok(buff::app::input::EventOutcome::Continue);
    };
    buff::app::input::execute_action(app, action)
}

fn run() -> Result<()> {
    let Some(cli) = parse_cli_args()? else {
        return Ok(());
    };

    let (config, notes_dir) = buff::config::load(cli.notes_dir).context("Config error")?;
    let mut app =
        buff::app::state::AppState::open_day(notes_dir, config, chrono::Local::now().date_naive())
            .context("Failed to open day")?;

    // LLM event channel: worker threads send LlmEvents; the loop polls them.
    let (llm_tx, llm_rx) = std::sync::mpsc::channel::<buff::app::llm::LlmEvent>();
    app.chat.event_tx = Some(llm_tx);

    let mut terminal = ratatui::init();
    let _guard = TerminalGuard;

    loop {
        let theme = buff::ui::theme::resolve_theme(&app.config.theme, &app.config.theme_overrides);
        terminal.draw(|frame| buff::ui::render(frame, &app, &theme))?;
        set_cursor_style(&app.focus)?;
        drain_llm_events(&mut app, &llm_rx);
        if process_key(&mut app)? == buff::app::input::EventOutcome::Quit {
            break;
        }
    }

    Ok(())
}
