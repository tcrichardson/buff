use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WeekStart {
    Sunday,
    Monday,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub notes_dir: Option<String>,
    pub timestamp_entries: bool,
    pub week_starts_on: WeekStart,
    pub date_format: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            notes_dir: None,
            timestamp_entries: false,
            week_starts_on: WeekStart::Sunday,
            date_format: "%Y-%m-%d-%a".to_string(),
        }
    }
}

pub fn config_path() -> PathBuf {
    match directories::ProjectDirs::from("", "", "kuatin") {
        Some(dirs) => dirs.config_dir().join("config.toml"),
        None => PathBuf::from(shellexpand::tilde("~/.config/kuatin/config.toml").as_ref()),
    }
}

pub fn default_notes_dir() -> PathBuf {
    if let Some(user_dirs) = directories::UserDirs::new()
        && let Some(docs) = user_dirs.document_dir()
    {
        return docs.join("kuatin");
    }
    PathBuf::from(shellexpand::tilde("~/kuatin").as_ref())
}

pub fn load(cli_notes_dir: Option<String>) -> anyhow::Result<(Config, PathBuf)> {
    let config = match std::fs::read_to_string(config_path()) {
        Ok(contents) => toml::from_str(&contents)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
        Err(e) => return Err(e.into()),
    };

    let notes_dir = resolve_notes_dir(
        cli_notes_dir,
        config.notes_dir.clone(),
        &default_notes_dir(),
    );

    Ok((config, notes_dir))
}

fn resolve_notes_dir(cli: Option<String>, cfg: Option<String>, default: &Path) -> PathBuf {
    match cli.or(cfg) {
        Some(path) => PathBuf::from(shellexpand::tilde(&path).as_ref()),
        None => default.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_toml() {
        let toml = r#"
            notes_dir = "/path/to/notes"
            timestamp_entries = true
            week_starts_on = "monday"
            date_format = "%d/%m/%Y"
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.notes_dir, Some("/path/to/notes".to_string()));
        assert!(config.timestamp_entries);
        assert_eq!(config.week_starts_on, WeekStart::Monday);
        assert_eq!(config.date_format, "%d/%m/%Y");
    }

    #[test]
    fn parse_missing_fields_uses_defaults() {
        let toml = r#"
            timestamp_entries = true
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.notes_dir, None);
        assert!(config.timestamp_entries);
        assert_eq!(config.week_starts_on, WeekStart::Sunday);
        assert_eq!(config.date_format, "%Y-%m-%d-%a");
    }

    #[test]
    fn resolve_notes_dir_precedence() {
        let default = Path::new("/default/kuatin");

        assert_eq!(
            resolve_notes_dir(Some("/cli/dir".to_string()), Some("/config/dir".to_string()), default),
            PathBuf::from("/cli/dir")
        );
        assert_eq!(
            resolve_notes_dir(None, Some("/config/dir".to_string()), default),
            PathBuf::from("/config/dir")
        );
        assert_eq!(
            resolve_notes_dir(None, None, default),
            PathBuf::from("/default/kuatin")
        );
    }

    #[test]
    fn resolve_notes_dir_tilde_expansion() {
        let default = Path::new("/default/kuatin");
        let result = resolve_notes_dir(Some("~/foo".to_string()), None, default);
        let home = std::env::var("HOME").expect("HOME not set");
        assert!(
            result.starts_with(&home),
            "expected {} to start with {}",
            result.display(),
            home
        );
        assert!(
            result.ends_with("foo"),
            "expected {} to end with foo",
            result.display()
        );
    }

}
