#![allow(dead_code)]

use chrono::{Datelike, NaiveDate};
use std::collections::BTreeSet;

use crate::config::WeekStart;

pub struct CalendarState {
    pub visible_month: (i32, u32), // (year, month)
    pub selected: NaiveDate,
}

impl CalendarState {
    pub fn new(focus_date: NaiveDate) -> Self {
        Self {
            visible_month: (focus_date.year(), focus_date.month()),
            selected: focus_date,
        }
    }
}

pub fn weeks(
    visible_month: (i32, u32),
    week_start: WeekStart,
) -> Vec<[Option<NaiveDate>; 7]> {
    let (year, month) = visible_month;
    let first_day = NaiveDate::from_ymd_opt(year, month, 1).unwrap();

    let weekday = first_day.weekday().num_days_from_sunday() as usize; // 0=Sun, 1=Mon, ...

    let offset = match week_start {
        WeekStart::Sunday => weekday,
        WeekStart::Monday => (weekday + 6) % 7,
    };

    let days_in_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    }
    .signed_duration_since(first_day)
    .num_days() as usize;

    let total_cells = (offset + days_in_month).div_ceil(7) * 7;
    let mut cells: Vec<Option<NaiveDate>> = Vec::with_capacity(total_cells);

    // leading padding
    for _ in 0..offset {
        cells.push(None);
    }

    // fill in dates
    for d in 0..days_in_month {
        cells.push(Some(first_day + chrono::Duration::days(d as i64)));
    }

    // trailing padding
    while !cells.len().is_multiple_of(7) {
        cells.push(None);
    }

    cells.chunks_exact(7).map(|w| {
        [
            w[0], w[1], w[2], w[3], w[4], w[5], w[6],
        ]
    }).collect()
}

pub fn move_selection(
    state: &mut CalendarState,
    dx_days: i64,
    dy_weeks: i64,
) {
    let delta = dx_days + dy_weeks * 7;
    state.selected += chrono::Duration::days(delta);
    state.visible_month = (state.selected.year(), state.selected.month());
}

/// Convenience wrapper for checking if a date is in a set.
pub fn marked(date: NaiveDate, dates: &BTreeSet<NaiveDate>) -> bool {
    dates.contains(&date)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn june_2026_sunday_start() {
        let june_2026 = (2026, 6);
        let grid = weeks(june_2026, WeekStart::Sunday);

        assert_eq!(grid.len(), 5, "expected 5 weeks for June 2026");

        // June 1, 2026 is a Monday; with Sunday start it should be in column 1
        let june_1 = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        assert_eq!(grid[0][1], Some(june_1));

        // First week column 0 should be None (leading padding)
        assert_eq!(grid[0][0], None);

        // Verify June 6 (Saturday) is in column 6
        let june_6 = NaiveDate::from_ymd_opt(2026, 6, 6).unwrap();
        assert_eq!(grid[0][6], Some(june_6));

        // Verify June 7 (Sunday) starts week 2, column 0
        let june_7 = NaiveDate::from_ymd_opt(2026, 6, 7).unwrap();
        assert_eq!(grid[1][0], Some(june_7));
    }

    #[test]
    fn june_2026_monday_start() {
        let june_2026 = (2026, 6);
        let grid = weeks(june_2026, WeekStart::Monday);

        assert_eq!(grid.len(), 5, "expected 5 weeks for June 2026");

        // June 1, 2026 is a Monday; with Monday start it should be in column 0
        let june_1 = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        assert_eq!(grid[0][0], Some(june_1));

        // June 7 (Sunday) should be in column 6
        let june_7 = NaiveDate::from_ymd_opt(2026, 6, 7).unwrap();
        assert_eq!(grid[0][6], Some(june_7));

        // First week column 1 should be June 2 (Tuesday)
        let june_2 = NaiveDate::from_ymd_opt(2026, 6, 2).unwrap();
        assert_eq!(grid[0][1], Some(june_2));
    }

    #[test]
    fn move_left_from_first_rolls_month() {
        let june_1 = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        let mut state = CalendarState::new(june_1);

        move_selection(&mut state, -1, 0);

        let may_31 = NaiveDate::from_ymd_opt(2026, 5, 31).unwrap();
        assert_eq!(state.selected, may_31);
        assert_eq!(state.visible_month, (2026, 5));
    }

    #[test]
    fn marked_reflects_set() {
        let mut dates = BTreeSet::new();
        let d1 = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        let d2 = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let d3 = NaiveDate::from_ymd_opt(2026, 6, 30).unwrap();

        dates.insert(d1);
        dates.insert(d3);

        assert!(marked(d1, &dates));
        assert!(!marked(d2, &dates));
        assert!(marked(d3, &dates));
    }
}
