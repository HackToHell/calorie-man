use crate::error::{bad, AppResult};
use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Deserializer};

pub fn parse_date(s: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
        .map_err(|_| bad(format!("invalid date '{s}', expected YYYY-MM-DD")))
        .and_then(|d| {
            if d.year() < 1900 || d.year() > 2200 {
                Err(bad(format!("date '{s}' is out of range")))
            } else {
                Ok(d)
            }
        })
}

pub fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// Today's date in the given IANA time zone, falling back to the server's local zone.
pub fn today_in(tz: Option<&str>) -> NaiveDate {
    let tz_name = tz
        .map(|s| s.to_string())
        .or_else(|| iana_time_zone::get_timezone().ok())
        .unwrap_or_else(|| "UTC".into());
    match tz_name.parse::<chrono_tz::Tz>() {
        Ok(tz) => Utc::now().with_timezone(&tz).date_naive(),
        Err(_) => chrono::Local::now().date_naive(),
    }
}

/// Resolve an optional date argument ("2024-05-01", "today", "yesterday", "-2") relative to `today`.
pub fn resolve_date(arg: Option<&str>, today: NaiveDate) -> AppResult<NaiveDate> {
    match arg.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(today),
        Some(s) => match s.to_lowercase().as_str() {
            "today" => Ok(today),
            "yesterday" => Ok(today - chrono::Duration::days(1)),
            "tomorrow" => Ok(today + chrono::Duration::days(1)),
            other => {
                if let Ok(n) = other.parse::<i64>() {
                    if n.abs() <= 3660 {
                        return Ok(today + chrono::Duration::days(n));
                    }
                }
                parse_date(s)
            }
        },
    }
}

/// Distinguish a missing field (None) from an explicit null (Some(None)) in PATCH-like inputs.
pub fn double_option<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// Parse a time of day: "7:40", "07:40", "19:40", "7:40 pm", "7pm", "12 AM", "noon", or "now".
/// Returns "HH:MM" (24-hour).
pub fn parse_time(raw: &str, now: chrono::NaiveTime) -> AppResult<String> {
    use chrono::Timelike;
    let s = raw.trim().to_lowercase().replace('.', "");
    let err = || {
        bad(format!(
            "invalid time '{raw}', use HH:MM like 07:40 or 19:30 (or '7:40 pm')"
        ))
    };
    match s.as_str() {
        "now" => return Ok(format!("{:02}:{:02}", now.hour(), now.minute())),
        "noon" => return Ok("12:00".into()),
        "midnight" => return Ok("00:00".into()),
        _ => {}
    }
    let (body, ampm) = if let Some(b) = s.strip_suffix("am") {
        (b.trim(), Some(false))
    } else if let Some(b) = s.strip_suffix("pm") {
        (b.trim(), Some(true))
    } else {
        (s.as_str(), None)
    };
    let mut parts = body.split(':');
    let h: u32 = parts
        .next()
        .ok_or_else(err)?
        .trim()
        .parse()
        .map_err(|_| err())?;
    let m: u32 = match parts.next() {
        Some(x) => x.trim().parse().map_err(|_| err())?,
        None => 0,
    };
    // Ignore seconds if given.
    if let Some(sec) = parts.next() {
        sec.trim().parse::<u32>().map_err(|_| err())?;
    }
    let h = match ampm {
        Some(pm) => {
            if !(1..=12).contains(&h) {
                return Err(err());
            }
            (h % 12) + if pm { 12 } else { 0 }
        }
        None => h,
    };
    if h > 23 || m > 59 {
        return Err(err());
    }
    Ok(format!("{h:02}:{m:02}"))
}

/// Minutes after midnight for "HH:MM".
pub fn minutes(hhmm: &str) -> Option<u32> {
    let (h, m) = hhmm.split_once(':')?;
    Some(h.parse::<u32>().ok()? * 60 + m.get(..2)?.parse::<u32>().ok()?)
}

pub fn hhmm(min: u32) -> String {
    format!("{:02}:{:02}", (min / 60) % 24, min % 60)
}

pub fn clean_opt(s: Option<String>) -> Option<String> {
    s.map(|x| x.trim().to_string()).filter(|x| !x.is_empty())
}

pub fn finite_pos(v: f64, what: &str) -> AppResult<f64> {
    if !v.is_finite() || v <= 0.0 {
        return Err(bad(format!("{what} must be a positive number")));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        let now = chrono::NaiveTime::from_hms_opt(16, 20, 0).unwrap();
        assert_eq!(parse_time("7:40", now).unwrap(), "07:40");
        assert_eq!(parse_time("19:30", now).unwrap(), "19:30");
        assert_eq!(parse_time("7:40 PM", now).unwrap(), "19:40");
        assert_eq!(parse_time("7pm", now).unwrap(), "19:00");
        assert_eq!(parse_time("12 am", now).unwrap(), "00:00");
        assert_eq!(parse_time("12:15pm", now).unwrap(), "12:15");
        assert_eq!(parse_time("now", now).unwrap(), "16:20");
        assert_eq!(parse_time("07:40:00", now).unwrap(), "07:40");
        assert!(parse_time("25:00", now).is_err());
        assert!(parse_time("13pm", now).is_err());
        assert!(parse_time("soon", now).is_err());
        assert_eq!(minutes("07:40"), Some(460));
        assert_eq!(hhmm(460), "07:40");
    }

    #[test]
    fn relative_dates() {
        let t = NaiveDate::from_ymd_opt(2024, 3, 1).unwrap();
        assert_eq!(resolve_date(None, t).unwrap(), t);
        assert_eq!(
            fmt_date(resolve_date(Some("yesterday"), t).unwrap()),
            "2024-02-29"
        );
        assert_eq!(fmt_date(resolve_date(Some("-2"), t).unwrap()), "2024-02-28");
        assert_eq!(
            fmt_date(resolve_date(Some("2024-01-05"), t).unwrap()),
            "2024-01-05"
        );
        assert!(resolve_date(Some("2024-13-05"), t).is_err());
        assert!(resolve_date(Some("soon"), t).is_err());
    }
}
