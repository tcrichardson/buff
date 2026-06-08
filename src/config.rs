use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WeekStart {
    Sunday,
    Monday,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaneSize {
    Columns(u16),
    Percent(u16),
}

impl<'de> serde::Deserialize<'de> for PaneSize {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct PaneSizeVisitor;
        impl<'de> serde::de::Visitor<'de> for PaneSizeVisitor {
            type Value = PaneSize;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "an integer column count or a percentage string like \"25%\"")
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<PaneSize, E> {
                Ok(PaneSize::Columns(v as u16))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<PaneSize, E> {
                Ok(PaneSize::Columns(v as u16))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<PaneSize, E> {
                if let Some(digits) = v.strip_suffix('%') {
                    digits.parse::<u16>()
                        .map(PaneSize::Percent)
                        .map_err(|_| E::custom(format!("invalid percentage: {}", v)))
                } else {
                    Err(E::custom(format!("expected integer or \"N%\" string, got: {}", v)))
                }
            }
        }
        d.deserialize_any(PaneSizeVisitor)
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub notes_dir: Option<String>,
    pub timestamp_entries: bool,
    pub week_starts_on: WeekStart,
    pub date_format: String,
    pub panel_width: PaneSize,
    pub todo_lookback_days: u16,
    pub capture_height: u16,
    pub llm_base_url: String,
    pub llm_model: String,
    pub llm_system_prompt: String,
    pub chat_visible: bool,
    pub theme: String,
    pub theme_overrides: ThemeOverrides,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            notes_dir: None,
            timestamp_entries: false,
            week_starts_on: WeekStart::Sunday,
            date_format: "%Y-%m-%d-%a".to_string(),
            panel_width: PaneSize::Columns(30),
            todo_lookback_days: 7,
            capture_height: 5,
            llm_base_url: "http://localhost:1234/v1".to_string(),
            llm_model: "google/gemma-4-12b-qat".to_string(),
            llm_system_prompt: String::new(),
            chat_visible: true,
            theme: "light".to_string(),
            theme_overrides: ThemeOverrides::default(),
        }
    }
}

pub fn config_path() -> PathBuf {
    PathBuf::from(shellexpand::tilde("~/.config/buff/config.toml").as_ref())
}

pub fn platform_config_path() -> PathBuf {
    match directories::ProjectDirs::from("", "", "buff") {
        Some(dirs) => dirs.config_dir().join("config.toml"),
        None => config_path(),
    }
}

pub fn default_notes_dir() -> PathBuf {
    if let Some(user_dirs) = directories::UserDirs::new()
        && let Some(docs) = user_dirs.document_dir()
    {
        return docs.join("buff");
    }
    PathBuf::from(shellexpand::tilde("~/buff").as_ref())
}

pub fn load(cli_notes_dir: Option<String>) -> anyhow::Result<(Config, PathBuf)> {
    let config = match std::fs::read_to_string(config_path()) {
        Ok(contents) => toml::from_str(&contents)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            match std::fs::read_to_string(platform_config_path()) {
                Ok(contents) => toml::from_str(&contents)?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
                Err(e) => return Err(e.into()),
            }
        }
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

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct ThemeOverrides {
    pub heading1: Option<String>,
    pub heading2: Option<String>,
    pub heading3: Option<String>,
    pub heading4: Option<String>,
    pub heading5: Option<String>,
    pub heading6: Option<String>,
    pub border_focused: Option<String>,
    pub border_unfocused: Option<String>,
    pub notes_panel_bg: Option<String>,
    pub panel_bg: Option<String>,
    pub chat_panel_bg: Option<String>,
    pub quote_marker: Option<String>,
    pub code: Option<String>,
    pub todo_done: Option<String>,
    pub todo_overdue: Option<String>,
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
        let default = Path::new("/default/buff");

        assert_eq!(
            resolve_notes_dir(
                Some("/cli/dir".to_string()),
                Some("/config/dir".to_string()),
                default
            ),
            PathBuf::from("/cli/dir")
        );
        assert_eq!(
            resolve_notes_dir(None, Some("/config/dir".to_string()), default),
            PathBuf::from("/config/dir")
        );
        assert_eq!(
            resolve_notes_dir(None, None, default),
            PathBuf::from("/default/buff")
        );
    }

    #[test]
    fn resolve_notes_dir_tilde_expansion() {
        let default = Path::new("/default/buff");
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

    #[test]
    fn panel_width_default_is_columns_30() {
        let config = Config::default();
        assert_eq!(config.panel_width, PaneSize::Columns(30));
    }

    #[test]
    fn panel_width_parses_as_integer() {
        let toml = r#"panel_width = 40"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.panel_width, PaneSize::Columns(40));
    }

    #[test]
    fn panel_width_parses_as_percentage_string() {
        let toml = r#"panel_width = "25%""#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.panel_width, PaneSize::Percent(25));
    }

    #[test]
    fn panel_width_percentage_100_is_valid() {
        let toml = r#"panel_width = "100%""#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.panel_width, PaneSize::Percent(100));
    }

    #[test]
    fn todo_lookback_days_default_is_7() {
        let config = Config::default();
        assert_eq!(config.todo_lookback_days, 7);
    }

    #[test]
    fn parse_panel_fields_from_toml() {
        let toml = r#"
            panel_width = 40
            todo_lookback_days = 14
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.panel_width, PaneSize::Columns(40));
        assert_eq!(config.todo_lookback_days, 14);
    }

    #[test]
    fn panel_fields_use_defaults_when_absent() {
        let toml = r#"timestamp_entries = true"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.panel_width, PaneSize::Columns(30));
        assert_eq!(config.todo_lookback_days, 7);
    }

    #[test]
    fn llm_and_chat_defaults() {
        let config = Config::default();
        assert_eq!(config.llm_base_url, "http://localhost:1234/v1");
        assert_eq!(config.llm_model, "google/gemma-4-12b-qat");
        assert_eq!(config.llm_system_prompt, "");
        assert!(config.chat_visible);
    }

    #[test]
    fn parse_llm_and_chat_fields_from_toml() {
        let toml = r#"
            llm_base_url = "http://127.0.0.1:9999/v1"
            llm_model = "my-model"
            llm_system_prompt = "be terse"
            chat_visible = false
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.llm_base_url, "http://127.0.0.1:9999/v1");
        assert_eq!(config.llm_model, "my-model");
        assert_eq!(config.llm_system_prompt, "be terse");
        assert!(!config.chat_visible);
    }

    #[test]
    fn theme_defaults_to_light() {
        let config = Config::default();
        assert_eq!(config.theme, "light");
    }

    #[test]
    fn parse_theme_name_from_toml() {
        let toml = r#"theme = "dark""#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.theme, "dark");
    }

    #[test]
    fn parse_theme_overrides_from_toml() {
        let toml = r##"
            theme = "light"
            [theme_overrides]
            heading1 = "red"
            border_focused = "#0000ff"
        "##;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.theme, "light");
        assert_eq!(config.theme_overrides.heading1, Some("red".to_string()));
        assert_eq!(config.theme_overrides.border_focused, Some("#0000ff".to_string()));
    }

    #[test]
    fn missing_theme_overrides_all_none() {
        let toml = r#"theme = "dark""#;
        let config: Config = toml::from_str(toml).unwrap();
        assert!(config.theme_overrides.heading1.is_none());
        assert!(config.theme_overrides.heading2.is_none());
        assert!(config.theme_overrides.panel_bg.is_none());
    }

    #[test]
    fn config_path_uses_xdg_location() {
        let path = config_path();
        assert!(path.to_string_lossy().contains(".config/buff/config.toml"));
    }
}

