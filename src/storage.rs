use chrono::NaiveDate;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub fn stem_for(date: NaiveDate, date_format: &str) -> String {
    date.format(date_format).to_string()
}

pub fn file_name_for(date: NaiveDate, date_format: &str) -> String {
    format!("{}.md", stem_for(date, date_format))
}

pub fn path_for(notes_dir: &Path, date: NaiveDate, date_format: &str) -> PathBuf {
    notes_dir.join(file_name_for(date, date_format))
}

pub fn note_exists(notes_dir: &Path, date: NaiveDate, date_format: &str) -> bool {
    path_for(notes_dir, date, date_format).exists()
}

pub fn dates_with_notes(notes_dir: &Path, date_format: &str) -> BTreeSet<NaiveDate> {
    let mut dates = BTreeSet::new();

    let Ok(entries) = std::fs::read_dir(notes_dir) else {
        return dates;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
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
}
