use crate::config::Config;
use crate::model::day::Document;
use crate::storage;
use chrono::NaiveDate;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Capture,
    Navigate,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Context {
    Notes,
    Meeting(usize),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Overlay {
    None,
    Calendar,
    Help,
}

pub struct AppState {
    pub doc: Document,
    pub date: NaiveDate,
    pub notes_dir: PathBuf,
    pub config: Config,
    pub context: Context,
    pub focus: Focus,
    pub selected: usize,
    pub status: String,
    pub input: String,
    pub overlay: Overlay,
    pub editing: Option<usize>,
    pub should_quit: bool,
}

impl AppState {
    pub fn open_day(notes_dir: PathBuf, config: Config, date: NaiveDate) -> anyhow::Result<Self> {
        let path = storage::path_for(&notes_dir, date, &config.date_format);
        let doc = if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            Document::from_text(&text)
        } else {
            Document::new_for_date(date)
        };
        Ok(Self {
            doc,
            date,
            notes_dir,
            config,
            context: Context::Notes,
            focus: Focus::Capture,
            selected: 0,
            status: String::new(),
            input: String::new(),
            overlay: Overlay::None,
            editing: None,
            should_quit: false,
        })
    }

    pub fn save(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.notes_dir)?;
        let path = storage::path_for(&self.notes_dir, self.date, &self.config.date_format);
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.doc.to_text())?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn current_time_hhmm(&self) -> String {
        chrono::Local::now().format("%H:%M").to_string()
    }
}
