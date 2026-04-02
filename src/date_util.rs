use chrono::NaiveDate;

pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let date_part = s.get(..10).unwrap_or(s);
    NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()
}

pub fn is_recent(date_str: &Option<String>, now: NaiveDate, days: i64) -> bool {
    match date_str {
        Some(ds) => parse_date(ds).map_or(false, |d| (now - d).num_days() <= days),
        None => false,
    }
}

pub fn date_is_newer(a: &str, b: &str) -> bool {
    match (parse_date(a), parse_date(b)) {
        (Some(da), Some(db)) => da > db,
        (Some(_), None) => true,
        _ => a > b,
    }
}
