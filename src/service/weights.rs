use crate::error::{bad, not_found, AppResult};
use crate::units::KG_PER_LB;
use crate::util::{clean_opt, double_option, fmt_date, parse_date};
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Weight {
    pub id: i64,
    pub user_id: i64,
    pub date: String,
    pub weight_kg: f64,
    pub body_fat_pct: Option<f64>,
    pub note: Option<String>,
}

fn from_row(r: &Row) -> rusqlite::Result<Weight> {
    Ok(Weight {
        id: r.get(0)?,
        user_id: r.get(1)?,
        date: r.get(2)?,
        weight_kg: r.get(3)?,
        body_fat_pct: r.get(4)?,
        note: r.get(5)?,
    })
}

const COLS: &str = "id, user_id, date, weight_kg, body_fat_pct, note";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightInput {
    pub date: Option<String>,
    pub weight: f64,
    /// "kg" or "lb"; defaults to the user's preferred unit.
    pub unit: Option<String>,
    pub body_fat_pct: Option<f64>,
    pub note: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightPatch {
    pub date: Option<String>,
    pub weight: Option<f64>,
    pub unit: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub body_fat_pct: Option<Option<f64>>,
    #[serde(default, deserialize_with = "double_option")]
    pub note: Option<Option<String>>,
}

fn check_not_future(date: &str, today: NaiveDate) -> AppResult<()> {
    // One day of slack for time-zone differences between the browser and the server.
    if parse_date(date)? > today + chrono::Duration::days(1) {
        return Err(bad("weigh-ins cannot be in the future"));
    }
    Ok(())
}

pub fn to_kg(v: f64, unit: &str) -> AppResult<f64> {
    let kg = match unit.trim().to_lowercase().as_str() {
        "kg" | "kgs" | "kilogram" | "kilograms" => v,
        "lb" | "lbs" | "pound" | "pounds" => v * KG_PER_LB,
        u => return Err(bad(format!("weight unit must be kg or lb, got '{u}'"))),
    };
    if !kg.is_finite() || !(20.0..=400.0).contains(&kg) {
        return Err(bad(format!(
            "weight {v} {unit} is outside the plausible range (20-400 kg)"
        )));
    }
    Ok(kg)
}

fn check_bf(v: Option<f64>) -> AppResult<Option<f64>> {
    if let Some(b) = v {
        if !b.is_finite() || !(2.0..=75.0).contains(&b) {
            return Err(bad("body_fat_pct must be between 2 and 75"));
        }
    }
    Ok(v)
}

/// Record a weigh-in. One per user per day: logging again on the same day replaces it.
pub fn upsert(
    c: &Connection,
    user_id: i64,
    default_unit: &str,
    inp: WeightInput,
    today: NaiveDate,
) -> AppResult<Weight> {
    let date = fmt_date(crate::util::resolve_date(inp.date.as_deref(), today)?);
    check_not_future(&date, today)?;
    let kg = to_kg(inp.weight, inp.unit.as_deref().unwrap_or(default_unit))?;
    let bf = check_bf(inp.body_fat_pct)?;
    let note = clean_opt(inp.note);
    c.execute(
        "INSERT INTO weights (user_id, date, weight_kg, body_fat_pct, note) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(user_id, date) DO UPDATE SET weight_kg = excluded.weight_kg,
           body_fat_pct = coalesce(excluded.body_fat_pct, weights.body_fat_pct), note = coalesce(excluded.note, weights.note)",
        params![user_id, date, (kg * 1000.0).round() / 1000.0, bf, note],
    )?;
    Ok(on_date(c, user_id, &date)?.expect("just inserted"))
}

pub fn get(c: &Connection, id: i64) -> AppResult<Weight> {
    c.query_row(
        &format!("SELECT {COLS} FROM weights WHERE id = ?"),
        [id],
        from_row,
    )
    .optional()?
    .ok_or_else(|| not_found(format!("weight {id} not found")))
}

pub fn update(c: &Connection, id: i64, default_unit: &str, p: WeightPatch) -> AppResult<Weight> {
    let w = get(c, id)?;
    let date = match p.date.as_deref() {
        Some(d) => fmt_date(parse_date(d)?),
        None => w.date.clone(),
    };
    check_not_future(&date, crate::util::today_in(None))?;
    let kg = match p.weight {
        Some(v) => to_kg(v, p.unit.as_deref().unwrap_or(default_unit))?,
        None => w.weight_kg,
    };
    let bf = match p.body_fat_pct {
        Some(v) => check_bf(v)?,
        None => w.body_fat_pct,
    };
    let note = match p.note {
        Some(n) => clean_opt(n),
        None => w.note.clone(),
    };
    if date != w.date && on_date(c, w.user_id, &date)?.is_some() {
        return Err(bad(format!("there is already a weigh-in on {date}")));
    }
    c.execute(
        "UPDATE weights SET date=?1, weight_kg=?2, body_fat_pct=?3, note=?4 WHERE id=?5",
        params![date, (kg * 1000.0).round() / 1000.0, bf, note, id],
    )?;
    get(c, id)
}

pub fn delete(c: &Connection, id: i64) -> AppResult<Weight> {
    let w = get(c, id)?;
    c.execute("DELETE FROM weights WHERE id = ?", [id])?;
    Ok(w)
}

pub fn on_date(c: &Connection, user_id: i64, date: &str) -> AppResult<Option<Weight>> {
    Ok(c.query_row(
        &format!("SELECT {COLS} FROM weights WHERE user_id = ? AND date = ?"),
        params![user_id, date],
        from_row,
    )
    .optional()?)
}

pub fn list(
    c: &Connection,
    user_id: i64,
    from: Option<&str>,
    to: Option<&str>,
) -> AppResult<Vec<Weight>> {
    let mut st = c.prepare(&format!(
        "SELECT {COLS} FROM weights WHERE user_id = ?1 AND date >= ?2 AND date <= ?3 ORDER BY date"
    ))?;
    let rows = st.query_map(
        params![
            user_id,
            from.unwrap_or("0000-00-00"),
            to.unwrap_or("9999-99-99")
        ],
        from_row,
    )?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn latest(c: &Connection, user_id: i64, on_or_before: &str) -> AppResult<Option<Weight>> {
    Ok(c.query_row(
        &format!(
            "SELECT {COLS} FROM weights WHERE user_id = ? AND date <= ? ORDER BY date DESC LIMIT 1"
        ),
        params![user_id, on_or_before],
        from_row,
    )
    .optional()?)
}

pub const TREND_ALPHA: f64 = 0.1;

/// Exponentially smoothed weight trend (Hacker's Diet style). Each weigh-in moves the trend 10% of
/// the way towards it; gaps between weigh-ins carry the trend forward. Returns (date, raw, trend).
pub fn trend(ws: &[Weight]) -> Vec<(String, f64, f64)> {
    let mut out = Vec::with_capacity(ws.len());
    let mut t: Option<f64> = None;
    let mut last_date: Option<NaiveDate> = None;
    for w in ws {
        let d = parse_date(&w.date).ok();
        let next = match (t, last_date, d) {
            (None, _, _) => w.weight_kg,
            (Some(prev), Some(ld), Some(d)) => {
                // Treat a gap of n days as n smoothing steps toward the new value.
                let gap = (d - ld).num_days().clamp(1, 30) as i32;
                let a = 1.0 - (1.0 - TREND_ALPHA).powi(gap);
                prev + a * (w.weight_kg - prev)
            }
            (Some(prev), _, _) => prev + TREND_ALPHA * (w.weight_kg - prev),
        };
        t = Some(next);
        last_date = d;
        out.push((
            w.date.clone(),
            w.weight_kg,
            (next * 1000.0).round() / 1000.0,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(date: &str, kg: f64) -> Weight {
        Weight {
            id: 0,
            user_id: 1,
            date: date.into(),
            weight_kg: kg,
            body_fat_pct: None,
            note: None,
        }
    }

    #[test]
    fn trend_smooths_and_handles_gaps() {
        let t = trend(&[
            w("2024-01-01", 80.0),
            w("2024-01-02", 81.0),
            w("2024-01-04", 79.0),
        ]);
        assert_eq!(t[0].2, 80.0);
        assert_eq!(t[1].2, 80.1);
        // two-day gap: a = 1 - 0.9^2 = 0.19
        assert!((t[2].2 - (80.1 + 0.19 * (79.0 - 80.1))).abs() < 0.001);
    }

    #[test]
    fn unit_conversion() {
        assert!((to_kg(176.0, "lb").unwrap() - 79.83).abs() < 0.01);
        assert!(to_kg(5.0, "kg").is_err());
        assert!(to_kg(80.0, "stone").is_err());
    }
}
