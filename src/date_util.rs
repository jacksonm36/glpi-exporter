use crate::models::{FilterState, RecentTimeMode};
use chrono::{NaiveDate, NaiveDateTime};

pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let date_part = s.get(..10).unwrap_or(s);
    NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()
}

pub fn parse_datetime(s: &str) -> Option<NaiveDateTime> {
    let t = s.trim().trim_end_matches('Z');
    NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S"))
        .or_else(|_| {
            let base = t.split('.').next().unwrap_or(t).trim();
            NaiveDateTime::parse_from_str(base, "%Y-%m-%dT%H:%M:%S")
                .or_else(|_| NaiveDateTime::parse_from_str(base, "%Y-%m-%d %H:%M:%S"))
        })
        .ok()
        .or_else(|| parse_date(s).and_then(|d| d.and_hms_opt(0, 0, 0)))
}

pub fn date_is_newer(a: &str, b: &str) -> bool {
    match (parse_date(a), parse_date(b)) {
        (Some(da), Some(db)) => da > db,
        (Some(_), None) => true,
        _ => a > b,
    }
}

/// Keep the chronologically earliest date (for “all hosts” floor).
pub fn merge_date_earliest(earliest: &mut Option<String>, candidate: &str) {
    let c = candidate.trim();
    if c.is_empty() {
        return;
    }
    match earliest {
        None => *earliest = Some(c.to_string()),
        Some(b) => {
            match (parse_date(c), parse_date(b)) {
                (Some(dc), Some(db)) => {
                    if dc < db {
                        *earliest = Some(c.to_string());
                    }
                }
                (Some(_), None) => *earliest = Some(c.to_string()),
                (None, Some(_)) => {}
                (None, None) => {
                    if c < b.as_str() {
                        *earliest = Some(c.to_string());
                    }
                }
            }
        }
    }
}

/// Inclusive range for Between mode (swaps ends if reversed).
pub fn recent_between_bounds(filters: &FilterState) -> (NaiveDate, NaiveDate) {
    let mut a = filters.recent_range_from;
    let mut b = filters.recent_range_to;
    if a > b {
        std::mem::swap(&mut a, &mut b);
    }
    (a, b)
}

/// Whether `date_str` falls in the active recency window (rolling / cutoff / between).
pub fn date_in_recency_window(
    date_str: &Option<String>,
    today: NaiveDate,
    filters: &FilterState,
) -> bool {
    let days: i64 = filters.days.parse().unwrap_or(30).max(0);
    let Some(ds) = date_str else {
        return false;
    };
    let Some(d) = parse_date(ds) else {
        return false;
    };
    if d > today {
        return false;
    }
    match filters.recent_time_mode {
        RecentTimeMode::RollingDays => (today - d).num_days() <= days,
        RecentTimeMode::CutoffFrom => d >= filters.recent_cutoff_from,
        RecentTimeMode::Between => {
            let (from, to) = recent_between_bounds(filters);
            d >= from && d <= to
        }
    }
}

/// Parsed event datetime within the filter window (inclusive of date portion for bounds).
pub fn event_in_recency_window(
    event_dt: NaiveDateTime,
    today: NaiveDate,
    filters: &FilterState,
) -> bool {
    let d = event_dt.date();
    if d > today {
        return false;
    }
    let days: i64 = filters.days.parse().unwrap_or(30).max(0);
    match filters.recent_time_mode {
        RecentTimeMode::RollingDays => (today - d).num_days() <= days,
        RecentTimeMode::CutoffFrom => d >= filters.recent_cutoff_from,
        RecentTimeMode::Between => {
            let (from, to) = recent_between_bounds(filters);
            d >= from && d <= to
        }
    }
}
