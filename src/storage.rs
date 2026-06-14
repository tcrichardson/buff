use chrono::NaiveDate;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub fn stem_for(date: NaiveDate, date_format: &str) -> String {
    date.format(date_format).to_string()
}

pub fn file_name_for(date: NaiveDate, date_format: &str) -> String {
    format!("{}.md", date.format(date_format))
}

pub fn path_for(notes_dir: &Path, date: NaiveDate, date_format: &str) -> PathBuf {
    notes_dir.join(file_name_for(date, date_format))
}

pub fn chat_path_for(notes_dir: &Path, date: NaiveDate, date_format: &str) -> PathBuf {
    notes_dir.join(format!("{}.chat.json", date.format(date_format)))
}

pub fn meeting_chat_path_for(
    notes_dir: &Path,
    date: NaiveDate,
    date_format: &str,
    ordinal: usize,
) -> PathBuf {
    notes_dir.join(format!(
        "{}-meeting{}.chat.json",
        date.format(date_format),
        ordinal
    ))
}

pub fn load_chat(path: &Path) -> Vec<crate::app::state::ChatMessage> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_chat(path: &Path, messages: &[crate::app::state::ChatMessage]) -> anyhow::Result<()> {
    if messages.is_empty() {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(messages)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn note_exists(notes_dir: &Path, date: NaiveDate, date_format: &str) -> bool {
    path_for(notes_dir, date, date_format).is_file()
}

pub fn dates_with_notes(notes_dir: &Path, date_format: &str) -> BTreeSet<NaiveDate> {
    let mut dates = BTreeSet::new();

    let Ok(entries) = std::fs::read_dir(notes_dir) else {
        return dates;
    };

    for entry in entries.filter_map(Result::ok) {
        if !entry.file_type().is_ok_and(|ft| ft.is_file()) {
            continue;
        }
        let name = entry.file_name();
        let path = Path::new(&name);
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Ok(date) = NaiveDate::parse_from_str(stem, date_format) {
            dates.insert(date);
        }
    }

    dates
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::fs;

    #[test]
    fn stem_for_formats_date() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        assert_eq!(stem_for(date, "%Y-%m-%d-%a"), "2026-06-04-Thu");
    }

    #[test]
    fn file_name_for_appends_md() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        assert_eq!(file_name_for(date, "%Y-%m-%d-%a"), "2026-06-04-Thu.md");
    }

    #[test]
    fn path_for_joins_notes_dir_and_file_name() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        let notes_dir = PathBuf::from("/tmp/notes");
        let expected = PathBuf::from("/tmp/notes/2026-06-04-Thu.md");
        assert_eq!(path_for(&notes_dir, date, "%Y-%m-%d-%a"), expected);
    }

    #[test]
    fn note_exists_returns_true_when_file_present() {
        let tmp = tempfile::tempdir().unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        let path = tmp.path().join("2026-06-04-Thu.md");
        fs::write(&path, "# Notes\n").unwrap();
        assert!(note_exists(tmp.path(), date, "%Y-%m-%d-%a"));
    }

    #[test]
    fn note_exists_returns_false_when_file_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        assert!(!note_exists(tmp.path(), date, "%Y-%m-%d-%a"));
    }

    #[test]
    fn roundtrip_parse_stem() {
        let original = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        let stem = stem_for(original, "%Y-%m-%d-%a");
        let parsed = NaiveDate::parse_from_str(&stem, "%Y-%m-%d-%a").unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn dates_with_notes_ignores_junk_and_collects_dates() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("2026-06-04-Thu.md"), "# Notes\n").unwrap();
        fs::write(tmp.path().join("2026-06-03-Wed.md"), "# Notes\n").unwrap();
        fs::write(tmp.path().join("junk.txt"), "junk\n").unwrap();

        let dates = dates_with_notes(tmp.path(), "%Y-%m-%d-%a");
        let expected: BTreeSet<NaiveDate> = [
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
            NaiveDate::from_ymd_opt(2026, 6, 4).unwrap(),
        ]
        .into_iter()
        .collect();
        assert_eq!(dates, expected);
    }

    #[test]
    fn dates_with_notes_returns_empty_for_empty_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let dates = dates_with_notes(tmp.path(), "%Y-%m-%d-%a");
        assert!(dates.is_empty());
    }

    #[test]
    fn dates_with_notes_ignores_directory_masquerading_as_md() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("2026-06-04-Thu.md")).unwrap();
        let dates = dates_with_notes(tmp.path(), "%Y-%m-%d-%a");
        assert!(dates.is_empty());
    }

    #[test]
    fn meeting_chat_path_for_uses_meeting_suffix() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();
        let dir = std::path::PathBuf::from("/tmp/notes");
        assert_eq!(
            meeting_chat_path_for(&dir, date, "%Y-%m-%d-%a", 0),
            std::path::PathBuf::from("/tmp/notes/2026-06-10-Wed-meeting0.chat.json")
        );
        assert_eq!(
            meeting_chat_path_for(&dir, date, "%Y-%m-%d-%a", 2),
            std::path::PathBuf::from("/tmp/notes/2026-06-10-Wed-meeting2.chat.json")
        );
    }

    #[test]
    fn chat_path_for_uses_chat_json_suffix() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 4).unwrap();
        let dir = std::path::PathBuf::from("/tmp/notes");
        assert_eq!(
            chat_path_for(&dir, date, "%Y-%m-%d-%a"),
            std::path::PathBuf::from("/tmp/notes/2026-06-04-Thu.chat.json")
        );
    }

    #[test]
    fn save_then_load_chat_roundtrip() {
        use crate::app::state::{ChatMessage, ChatRole};
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("c.chat.json");
        let msgs = vec![
            ChatMessage {
                role: ChatRole::User,
                content: "q".to_string(),
            },
            ChatMessage {
                role: ChatRole::Assistant,
                content: "a".to_string(),
            },
        ];
        save_chat(&path, &msgs).unwrap();
        assert_eq!(load_chat(&path), msgs);
    }

    #[test]
    fn save_empty_removes_file() {
        use crate::app::state::ChatMessage;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("c.chat.json");
        std::fs::write(&path, "[]").unwrap();
        let empty: Vec<ChatMessage> = vec![];
        save_chat(&path, &empty).unwrap();
        assert!(
            !path.exists(),
            "empty conversation should remove the sidecar"
        );
    }

    #[test]
    fn load_missing_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nope.chat.json");
        assert!(load_chat(&path).is_empty());
    }

    #[test]
    fn load_malformed_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("bad.chat.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(load_chat(&path).is_empty());
    }
}
