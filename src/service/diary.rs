use crate::error::{bad, not_found, AppResult};
use crate::nutrients::{self, Nutrients, CATALOG};
use crate::service::foods::{self, fmt_num};
use crate::service::goals::{self, Goal, Target};
use crate::service::weights::{self, Weight};
use crate::util::{clean_opt, double_option, fmt_date, parse_date};
use chrono::{Datelike, NaiveDate};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MEALS: &[&str] = &["breakfast", "lunch", "dinner", "snacks"];

pub fn normalize_meal(m: Option<&str>) -> AppResult<Option<&'static str>> {
    let Some(m) = m.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let found = match m.as_str() {
        "breakfast" | "morning" => "breakfast",
        "lunch" | "noon" => "lunch",
        "dinner" | "supper" | "evening" => "dinner",
        "snack" | "snacks" | "other" => "snacks",
        _ => return Err(bad(format!("meal must be one of {}", MEALS.join(", ")))),
    };
    Ok(Some(found))
}

/// Default meal from the time of day when none is given.
pub fn meal_for_hour(h: u32) -> &'static str {
    match h {
        4..=10 => "breakfast",
        11..=15 => "lunch",
        16..=21 => "dinner",
        _ => "snacks",
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub id: i64,
    pub user_id: i64,
    pub date: String,
    pub meal: String,
    /// When it was eaten: "HH:MM" in the user's local time, if known.
    pub time: Option<String>,
    pub food_id: Option<i64>,
    pub name: String,
    pub brand: Option<String>,
    pub quantity: f64,
    pub unit: String,
    pub serving_id: Option<i64>,
    /// Amount in the food's basis unit (for display like "480 ml").
    pub amount: Option<f64>,
    pub note: Option<String>,
    pub source: String,
    pub created_at: String,
    pub nutrients: Nutrients,
    /// "2 cup (480 ml)"
    pub display_quantity: String,
}

const ENTRY_COLS: &str =
    "e.id, e.user_id, e.date, e.meal, e.food_id, e.name, e.brand, e.quantity, e.unit, e.serving_id, e.amount, e.note, e.source, e.created_at, f.basis_unit, e.eaten_at";
const E_N_OFFSET: usize = 16;

fn entry_sql() -> String {
    format!(
        "SELECT {ENTRY_COLS}, {} FROM entries e LEFT JOIN foods f ON f.id = e.food_id",
        nutrients::columns("e.")
    )
}

fn entry_from_row(r: &Row) -> rusqlite::Result<Entry> {
    let quantity: f64 = r.get(7)?;
    let unit: String = r.get(8)?;
    let amount: Option<f64> = r.get(10)?;
    let basis: Option<String> = r.get(14)?;
    let food_id: Option<i64> = r.get(4)?;
    Ok(Entry {
        id: r.get(0)?,
        user_id: r.get(1)?,
        date: r.get(2)?,
        meal: r.get(3)?,
        time: r.get(15)?,
        display_quantity: display_quantity(
            food_id.is_some(),
            quantity,
            &unit,
            amount,
            basis.as_deref(),
        ),
        food_id,
        name: r.get(5)?,
        brand: r.get(6)?,
        quantity,
        unit,
        serving_id: r.get(9)?,
        amount,
        note: r.get(11)?,
        source: r.get(12)?,
        created_at: r.get(13)?,
        nutrients: Nutrients::from_row(r, E_N_OFFSET)?,
    })
}

pub fn display_quantity(
    has_food: bool,
    qty: f64,
    unit: &str,
    amount: Option<f64>,
    basis: Option<&str>,
) -> String {
    if !has_food && unit.is_empty() {
        return if qty == 1.0 {
            String::new()
        } else {
            format!("× {}", fmt_num(qty))
        };
    }
    let q = fmt_num(qty);
    let unit = if unit == "serving" && qty != 1.0 {
        "servings"
    } else {
        unit
    };
    let main = if unit
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        format!("{q} × {unit}")
    } else {
        format!("{q} {unit}")
    };
    match (amount, basis) {
        (Some(a), Some(b)) if (b == "g" || b == "ml") && unit != b => {
            format!("{main} ({} {b})", fmt_num(a))
        }
        _ => main,
    }
}

pub fn get_entry(c: &Connection, id: i64) -> AppResult<Entry> {
    c.query_row(
        &format!("{} WHERE e.id = ?", entry_sql()),
        [id],
        entry_from_row,
    )
    .optional()?
    .ok_or_else(|| not_found(format!("entry {id} not found")))
}

pub fn entries_for(c: &Connection, user_id: i64, date: &str) -> AppResult<Vec<Entry>> {
    let mut st = c.prepare(&format!(
        "{} WHERE e.user_id = ? AND e.date = ?
         ORDER BY CASE e.meal WHEN 'breakfast' THEN 0 WHEN 'lunch' THEN 1 WHEN 'dinner' THEN 2 ELSE 3 END, coalesce(e.eaten_at, '99:99'), e.created_at, e.id",
        entry_sql()
    ))?;
    let rows = st.query_map(params![user_id, date], entry_from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryInput {
    pub date: Option<String>,
    pub meal: Option<String>,
    pub food_id: Option<i64>,
    pub quantity: Option<f64>,
    pub unit: Option<String>,
    pub serving_id: Option<i64>,
    pub note: Option<String>,
    /// Quick add only: a name and nutrient totals, no food.
    pub name: Option<String>,
    pub nutrients: Option<Nutrients>,
    pub source: Option<String>,
    /// When it was eaten ("19:30", "7:30 pm", "now"). Missing: now, for today's entries whose meal fits
    /// the time of day; otherwise unknown. `null`: unknown.
    #[serde(default, deserialize_with = "double_option")]
    pub time: Option<Option<String>>,
}

fn write_entry_nutrients(c: &Connection, id: i64, n: &Nutrients) -> AppResult<()> {
    let sets = CATALOG
        .iter()
        .enumerate()
        .map(|(i, x)| format!("n_{} = ?{}", x.key, i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let mut p: Vec<rusqlite::types::Value> = n.params().into_iter().map(|v| v.into()).collect();
    p.push(id.into());
    c.execute(
        &format!(
            "UPDATE entries SET {sets} WHERE id = ?{}",
            CATALOG.len() + 1
        ),
        params_from_iter(p),
    )?;
    Ok(())
}

fn check_note(n: Option<String>) -> AppResult<Option<String>> {
    let n = clean_opt(n);
    if n.as_ref().map(|s| s.chars().count() > 500).unwrap_or(false) {
        return Err(bad("note is too long (max 500 characters)"));
    }
    Ok(n)
}

/// Log a food (by id and quantity) or a quick-add entry (name + nutrients).
pub fn add_entry(
    c: &Connection,
    user_id: i64,
    inp: EntryInput,
    today: NaiveDate,
    now: chrono::NaiveTime,
) -> AppResult<Entry> {
    use chrono::Timelike;
    let date = fmt_date(crate::util::resolve_date(inp.date.as_deref(), today)?);
    let meal_given = normalize_meal(inp.meal.as_deref())?;
    let meal = meal_given.unwrap_or_else(|| meal_for_hour(now.hour()));
    let time = match inp.time {
        Some(Some(t)) if !t.trim().is_empty() => Some(crate::util::parse_time(&t, now)?),
        Some(_) => None,
        // Logging breakfast at 9 pm is a back-fill, so "now" would be wrong; leave it unknown.
        None => (date == fmt_date(today)
            && meal_given.is_none_or(|m| m == meal_for_hour(now.hour())))
        .then(|| format!("{:02}:{:02}", now.hour(), now.minute())),
    };
    let note = check_note(inp.note)?;
    let source = clean_opt(inp.source).unwrap_or_else(|| "web".into());
    let id = match inp.food_id {
        Some(fid) => {
            let food = foods::get(c, fid)?;
            let qty = inp.quantity.unwrap_or(1.0);
            let r = foods::resolve_amount(&food, qty, inp.unit.as_deref(), inp.serving_id)?;
            c.execute(
                "INSERT INTO entries (user_id, date, meal, food_id, name, brand, quantity, unit, serving_id, amount, note, source, eaten_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![user_id, date, meal, fid, food.name, food.brand, qty, r.unit, r.serving_id, r.amount, note, source, time],
            )?;
            let id = c.last_insert_rowid();
            write_entry_nutrients(c, id, &food.nutrients_for(r.amount))?;
            id
        }
        None => {
            let name = clean_opt(inp.name).unwrap_or_else(|| "Quick add".into());
            if name.chars().count() > 200 {
                return Err(bad("name is too long"));
            }
            let n = inp.nutrients.unwrap_or_default();
            if n.is_empty() {
                return Err(bad("give either food_id, or a name with nutrients (at least calories) for a quick add"));
            }
            let qty = inp.quantity.unwrap_or(1.0);
            crate::util::finite_pos(qty, "quantity")?;
            c.execute(
                "INSERT INTO entries (user_id, date, meal, food_id, name, quantity, unit, note, source, eaten_at)
                 VALUES (?1, ?2, ?3, NULL, ?4, ?5, '', ?6, ?7, ?8)",
                params![user_id, date, meal, name, qty, note, source, time],
            )?;
            let id = c.last_insert_rowid();
            write_entry_nutrients(c, id, &n.scale(qty).rounded())?;
            id
        }
    };
    get_entry(c, id)
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryPatch {
    pub date: Option<String>,
    pub meal: Option<String>,
    pub quantity: Option<f64>,
    pub unit: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub serving_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "double_option")]
    pub note: Option<Option<String>>,
    pub name: Option<String>,
    /// Quick-add entries only: replaces the nutrient totals (for quantity 1).
    pub nutrients: Option<Nutrients>,
    /// "19:30"; `null` clears the time.
    #[serde(default, deserialize_with = "double_option")]
    pub time: Option<Option<String>>,
}

pub fn update_entry(c: &Connection, id: i64, p: EntryPatch) -> AppResult<Entry> {
    let e = get_entry(c, id)?;
    let date = match p.date.as_deref() {
        Some(d) => fmt_date(parse_date(d)?),
        None => e.date.clone(),
    };
    let meal = normalize_meal(p.meal.as_deref())?
        .map(|s| s.to_string())
        .unwrap_or(e.meal.clone());
    let note = match p.note {
        Some(n) => check_note(n)?,
        None => e.note.clone(),
    };
    let qty = p.quantity.unwrap_or(e.quantity);
    if !qty.is_finite() || qty <= 0.0 || qty > 100_000.0 {
        return Err(bad("quantity must be a positive number"));
    }
    let time = match &p.time {
        Some(Some(t)) if !t.trim().is_empty() => Some(Some(crate::util::parse_time(
            t,
            chrono::Local::now().time(),
        )?)),
        Some(_) => Some(None),
        None => None,
    };
    let food = match e.food_id {
        Some(fid) => Some(foods::get(c, fid)?),
        None => None,
    };
    match food {
        Some(food) => {
            if p.nutrients.is_some() {
                return Err(bad(
                    "nutrients can only be edited on quick-add entries; edit the food instead",
                ));
            }
            let (unit, sid) = if p.unit.is_some() || p.serving_id.is_some() {
                (p.unit.clone(), p.serving_id.flatten())
            } else {
                (Some(e.unit.clone()), e.serving_id)
            };
            let unit_changed = p.unit.is_some() || p.serving_id.is_some();
            let resolved = foods::resolve_amount(&food, qty, unit.as_deref(), sid).or_else(|err| {
                if sid.is_some() {
                    foods::resolve_amount(&food, qty, unit.as_deref(), None)
                } else {
                    Err(err)
                }
            });
            match resolved {
                Ok(r) => {
                    c.execute(
                        "UPDATE entries SET date=?1, meal=?2, quantity=?3, unit=?4, serving_id=?5, amount=?6, note=?7 WHERE id=?8",
                        params![date, meal, qty, r.unit, r.serving_id, r.amount, note, id],
                    )?;
                    write_entry_nutrients(c, id, &food.nutrients_for(r.amount))?;
                }
                // The unit it was logged with no longer exists on the food (a deleted serving, a changed
                // basis). Keep the entry as logged and scale it, rather than refusing the edit.
                Err(_) if !unit_changed && e.quantity > 0.0 => {
                    let f = qty / e.quantity;
                    c.execute(
                        "UPDATE entries SET date=?1, meal=?2, quantity=?3, serving_id=NULL, amount=?4, note=?5 WHERE id=?6",
                        params![date, meal, qty, e.amount.map(|a| a * f), note, id],
                    )?;
                    write_entry_nutrients(c, id, &e.nutrients.scale(f).rounded())?;
                }
                Err(err) => return Err(err),
            }
        }
        None => {
            let name = match p.name {
                Some(n) => clean_opt(Some(n)).ok_or_else(|| bad("name cannot be empty"))?,
                None => e.name.clone(),
            };
            let n = match p.nutrients {
                Some(n) => {
                    if n.is_empty() {
                        return Err(bad("a quick-add entry needs at least one nutrient"));
                    }
                    n.scale(qty)
                }
                None => e.nutrients.scale(qty / e.quantity),
            };
            c.execute(
                "UPDATE entries SET date=?1, meal=?2, quantity=?3, note=?4, name=?5 WHERE id=?6",
                params![date, meal, qty, note, name, id],
            )?;
            write_entry_nutrients(c, id, &n.rounded())?;
        }
    }
    if let Some(t) = time {
        c.execute(
            "UPDATE entries SET eaten_at = ? WHERE id = ?",
            params![t, id],
        )?;
    }
    get_entry(c, id)
}

pub fn delete_entry(c: &Connection, id: i64) -> AppResult<Entry> {
    let e = get_entry(c, id)?;
    c.execute("DELETE FROM entries WHERE id = ?", [id])?;
    Ok(e)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopyInput {
    pub from_date: String,
    pub to_date: String,
    pub meal: Option<String>,
    /// Put the copies into this meal instead of their original meal.
    pub to_meal: Option<String>,
    /// Copy from another user's diary (e.g. a shared dinner).
    pub from_user_id: Option<i64>,
}

pub fn copy_entries(c: &mut Connection, user_id: i64, inp: CopyInput) -> AppResult<Vec<Entry>> {
    let from = fmt_date(parse_date(&inp.from_date)?);
    let to = fmt_date(parse_date(&inp.to_date)?);
    let meal = normalize_meal(inp.meal.as_deref())?;
    let to_meal = normalize_meal(inp.to_meal.as_deref())?;
    let src_user = inp.from_user_id.unwrap_or(user_id);
    crate::service::users::get(c, src_user)?;
    if src_user == user_id && from == to && (to_meal.is_none() || to_meal == meal) {
        return Err(bad("source and destination are the same"));
    }
    let tx = c.transaction()?;
    let src: Vec<Entry> = entries_for(&tx, src_user, &from)?
        .into_iter()
        .filter(|e| meal.map(|m| e.meal == m).unwrap_or(true))
        .collect();
    let ncols = nutrients::columns("");
    let mut ids = vec![];
    for e in &src {
        tx.execute(
            &format!(
                "INSERT INTO entries (user_id, date, meal, food_id, name, brand, quantity, unit, serving_id, amount, note, source, eaten_at, {ncols})
                 SELECT ?1, ?2, ?3, food_id, name, brand, quantity, unit, serving_id, amount, note, 'copy', eaten_at, {ncols}
                 FROM entries WHERE id = ?4"
            ),
            params![user_id, to, to_meal.unwrap_or(e.meal.as_str()), e.id],
        )?;
        ids.push(tx.last_insert_rowid());
    }
    let mut out = vec![];
    for id in ids {
        out.push(get_entry(&tx, id)?);
    }
    tx.commit()?;
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------
// Exercise

#[derive(Debug, Clone, Serialize)]
pub struct Exercise {
    pub id: i64,
    pub user_id: i64,
    pub date: String,
    pub name: String,
    pub calories: f64,
    pub duration_min: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExerciseInput {
    pub date: Option<String>,
    pub name: String,
    pub calories: f64,
    pub duration_min: Option<f64>,
}

fn exercise_from_row(r: &Row) -> rusqlite::Result<Exercise> {
    Ok(Exercise {
        id: r.get(0)?,
        user_id: r.get(1)?,
        date: r.get(2)?,
        name: r.get(3)?,
        calories: r.get(4)?,
        duration_min: r.get(5)?,
    })
}

pub fn add_exercise(
    c: &Connection,
    user_id: i64,
    inp: ExerciseInput,
    today: NaiveDate,
) -> AppResult<Exercise> {
    let date = fmt_date(crate::util::resolve_date(inp.date.as_deref(), today)?);
    let name = inp.name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(bad("exercise name must be 1-100 characters"));
    }
    if !inp.calories.is_finite() || inp.calories < 0.0 || inp.calories > 10000.0 {
        return Err(bad("exercise calories must be between 0 and 10000"));
    }
    if let Some(d) = inp.duration_min {
        if !d.is_finite() || d < 0.0 || d > 1440.0 {
            return Err(bad("duration_min must be between 0 and 1440"));
        }
    }
    c.execute(
        "INSERT INTO exercises (user_id, date, name, calories, duration_min) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![user_id, date, name, inp.calories, inp.duration_min],
    )?;
    let id = c.last_insert_rowid();
    Ok(c.query_row(
        "SELECT id, user_id, date, name, calories, duration_min FROM exercises WHERE id = ?",
        [id],
        exercise_from_row,
    )?)
}

pub fn exercises_between(
    c: &Connection,
    user_id: i64,
    from: &str,
    to: &str,
) -> AppResult<Vec<Exercise>> {
    let mut st = c.prepare(
        "SELECT id, user_id, date, name, calories, duration_min FROM exercises WHERE user_id = ? AND date BETWEEN ? AND ? ORDER BY date, id",
    )?;
    let rows = st.query_map(params![user_id, from, to], exercise_from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn delete_exercise(c: &Connection, id: i64) -> AppResult<()> {
    if c.execute("DELETE FROM exercises WHERE id = ?", [id])? == 0 {
        return Err(not_found(format!("exercise {id} not found")));
    }
    Ok(())
}

pub fn exercise_owner(c: &Connection, id: i64) -> AppResult<i64> {
    c.query_row("SELECT user_id FROM exercises WHERE id = ?", [id], |r| {
        r.get(0)
    })
    .optional()?
    .ok_or_else(|| not_found(format!("exercise {id} not found")))
}

// ---------------------------------------------------------------------------------------------------------
// Daily totals

/// Per-day nutrient totals for a date range, only for days with entries.
pub fn daily_totals(
    c: &Connection,
    user_id: i64,
    from: &str,
    to: &str,
) -> AppResult<BTreeMap<String, (Nutrients, i64)>> {
    let sums = CATALOG
        .iter()
        .map(|n| format!("SUM(n_{})", n.key))
        .collect::<Vec<_>>()
        .join(", ");
    let mut st = c.prepare(&format!(
        "SELECT date, COUNT(*), {sums} FROM entries WHERE user_id = ? AND date BETWEEN ? AND ? GROUP BY date"
    ))?;
    let rows = st.query_map(params![user_id, from, to], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            Nutrients::from_row(r, 2)?,
        ))
    })?;
    let mut out = BTreeMap::new();
    for row in rows {
        let (d, n, nut) = row?;
        out.insert(d, (nut.rounded(), n));
    }
    Ok(out)
}

pub fn exercise_totals(
    c: &Connection,
    user_id: i64,
    from: &str,
    to: &str,
) -> AppResult<BTreeMap<String, f64>> {
    let mut st = c.prepare(
        "SELECT date, SUM(calories) FROM exercises WHERE user_id = ? AND date BETWEEN ? AND ? GROUP BY date",
    )?;
    let rows = st.query_map(params![user_id, from, to], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn budget_for(goal: Option<&Goal>, exercise: f64) -> Option<f64> {
    let g = goal?;
    let cal = g.calories?;
    Some(if g.exercise_adds_to_budget {
        cal + exercise
    } else {
        cal
    })
}

#[derive(Debug, Serialize)]
pub struct Budget {
    pub goal: Option<f64>,
    pub exercise: f64,
    pub exercise_adds_to_budget: bool,
    pub budget: Option<f64>,
    pub eaten: f64,
    pub remaining: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct Week {
    pub start: String,
    pub end: String,
    pub logged_days: usize,
    /// Sum over logged days of this week *before* the selected day of (budget - eaten). Positive means
    /// calories banked (under budget), negative means overspent.
    pub balance: f64,
    pub weekly_budget: Option<f64>,
    pub eaten: f64,
    pub weekly_remaining: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct Diary {
    pub date: String,
    pub user_id: i64,
    pub entries: Vec<Entry>,
    pub exercises: Vec<Exercise>,
    pub totals: Nutrients,
    pub meals: BTreeMap<String, Nutrients>,
    pub goal: Option<Goal>,
    pub targets: Vec<Target>,
    pub budget: Budget,
    pub week: Week,
    pub weight: Option<Weight>,
    /// For each nutrient, the share of entries that report it (1.0 = every entry). Low coverage means
    /// the total is an underestimate.
    pub coverage: BTreeMap<String, f64>,
    pub streak: Streak,
    pub timing: Timing,
}

/// One sitting: entries of the same meal eaten within an hour of each other.
#[derive(Debug, Clone, Serialize)]
pub struct Occasion {
    pub meal: String,
    /// Time of the first entry, "HH:MM".
    pub time: String,
    pub calories: f64,
    pub protein: f64,
    pub entries: usize,
}

#[derive(Debug, Serialize)]
pub struct Timing {
    pub occasions: Vec<Occasion>,
    /// Typical time of each meal: the median time of its first entry over the 28 days before this one
    /// (needs at least 3 days with a time).
    pub usual: BTreeMap<String, String>,
    /// First and last eating times of the day.
    pub first: Option<String>,
    pub last: Option<String>,
    /// Entries on this day without a time.
    pub untimed: usize,
}

pub fn occasions(entries: &[Entry]) -> Vec<Occasion> {
    let mut timed: Vec<(&Entry, u32)> = entries
        .iter()
        .filter_map(|e| {
            e.time
                .as_deref()
                .and_then(crate::util::minutes)
                .map(|m| (e, m))
        })
        .collect();
    timed.sort_by_key(|(_, m)| *m);
    let mut out: Vec<(Occasion, u32)> = vec![];
    for (e, m) in timed {
        let slot = out
            .iter_mut()
            .find(|(o, start)| o.meal == e.meal && m - *start <= 60);
        let (cal, pro) = (e.nutrients.val("calories"), e.nutrients.val("protein"));
        match slot {
            Some((o, _)) => {
                o.calories += cal;
                o.protein += pro;
                o.entries += 1;
            }
            None => out.push((
                Occasion {
                    meal: e.meal.clone(),
                    time: crate::util::hhmm(m),
                    calories: cal,
                    protein: pro,
                    entries: 1,
                },
                m,
            )),
        }
    }
    out.into_iter()
        .map(|(mut o, _)| {
            o.calories = nutrients::round_to(o.calories, 1);
            o.protein = nutrients::round_to(o.protein, 1);
            o
        })
        .collect()
}

pub fn usual_times(
    c: &Connection,
    user_id: i64,
    date: NaiveDate,
) -> AppResult<BTreeMap<String, String>> {
    let from = fmt_date(date - chrono::Duration::days(28));
    let mut st = c.prepare(
        "SELECT meal, MIN(eaten_at) FROM entries WHERE user_id = ? AND date >= ? AND date < ? AND eaten_at IS NOT NULL
         GROUP BY meal, date",
    )?;
    let rows = st.query_map(params![user_id, from, fmt_date(date)], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut by_meal: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for row in rows {
        let (meal, t) = row?;
        if let Some(m) = crate::util::minutes(&t) {
            by_meal.entry(meal).or_default().push(m);
        }
    }
    Ok(by_meal
        .into_iter()
        .filter(|(_, v)| v.len() >= 3)
        .map(|(meal, mut v)| {
            v.sort_unstable();
            let n = v.len();
            let med = if n % 2 == 1 {
                v[n / 2]
            } else {
                (v[n / 2 - 1] + v[n / 2]) / 2
            };
            (meal, crate::util::hhmm(med))
        })
        .collect())
}

/// Consecutive complete days ending the day before `date` (the day itself is still in progress).
#[derive(Debug, Serialize)]
pub struct Streak {
    /// Days that were logged and stayed within the calorie budget.
    pub on_budget: u32,
    /// Days with at least one entry.
    pub logged: u32,
}

pub fn streak(c: &Connection, user_id: i64, date: NaiveDate) -> AppResult<Streak> {
    let from = fmt_date(date - chrono::Duration::days(400));
    let to = fmt_date(date - chrono::Duration::days(1));
    let totals = daily_totals(c, user_id, &from, &to)?;
    let ex = exercise_totals(c, user_id, &from, &to)?;
    let goals_all = goals::list(c, user_id)?;
    let (mut on_budget, mut logged, mut budget_run) = (0u32, 0u32, true);
    for i in 1..=400 {
        let d = fmt_date(date - chrono::Duration::days(i));
        let Some((n, _)) = totals.get(&d) else { break };
        logged += 1;
        if budget_run {
            let b = budget_for(
                goals::pick(&goals_all, &d),
                ex.get(&d).copied().unwrap_or(0.0),
            );
            match b {
                Some(b) if n.val("calories") <= b => on_budget += 1,
                _ => budget_run = false,
            }
        }
    }
    Ok(Streak { on_budget, logged })
}

pub fn diary(c: &Connection, user_id: i64, date: NaiveDate) -> AppResult<Diary> {
    let d = fmt_date(date);
    let entries = entries_for(c, user_id, &d)?;
    let exercises = exercises_between(c, user_id, &d, &d)?;
    let mut totals = Nutrients::empty();
    let mut meals: BTreeMap<String, Nutrients> = MEALS
        .iter()
        .map(|m| (m.to_string(), Nutrients::empty()))
        .collect();
    let mut coverage = BTreeMap::new();
    for e in &entries {
        totals.add(&e.nutrients);
        if let Some(m) = meals.get_mut(&e.meal) {
            m.add(&e.nutrients);
        }
    }
    if !entries.is_empty() {
        for (i, n) in CATALOG.iter().enumerate() {
            let known = entries
                .iter()
                .filter(|e| e.nutrients.0[i].is_some())
                .count();
            coverage.insert(
                n.key.to_string(),
                nutrients::round_to(known as f64 / entries.len() as f64, 3),
            );
        }
    }
    let goals_all = goals::list(c, user_id)?;
    let goal = goals::pick(&goals_all, &d).cloned();
    let ex_total: f64 = exercises.iter().fold(0.0, |a, e| a + e.calories);
    let eaten = totals.val("calories");
    let budget_v = budget_for(goal.as_ref(), ex_total);
    let budget = Budget {
        goal: goal.as_ref().and_then(|g| g.calories),
        exercise: ex_total,
        exercise_adds_to_budget: goal
            .as_ref()
            .map(|g| g.exercise_adds_to_budget)
            .unwrap_or(false),
        budget: budget_v,
        eaten: nutrients::round_to(eaten, 1),
        remaining: budget_v.map(|b| nutrients::round_to(b - eaten, 1)),
    };
    // Week (Monday start).
    let start = date - chrono::Duration::days(date.weekday().num_days_from_monday() as i64);
    let end = start + chrono::Duration::days(6);
    let (ss, es) = (fmt_date(start), fmt_date(end));
    let totals_w = daily_totals(c, user_id, &ss, &es)?;
    let ex_w = exercise_totals(c, user_id, &ss, &es)?;
    let mut balance = 0.0;
    let mut logged = 0;
    let mut eaten_w = 0.0;
    let mut weekly_budget = Some(0.0);
    for i in 0..7 {
        let day = fmt_date(start + chrono::Duration::days(i));
        let g = goals::pick(&goals_all, &day);
        let ex = ex_w.get(&day).copied().unwrap_or(0.0);
        let b = budget_for(g, ex);
        weekly_budget = match (weekly_budget, b) {
            (Some(w), Some(b)) => Some(w + b),
            _ => None,
        };
        if let Some((n, _)) = totals_w.get(&day) {
            eaten_w += n.val("calories");
            if day <= d {
                logged += 1;
            }
            if day < d {
                if let Some(b) = b {
                    balance += b - n.val("calories");
                }
            }
        }
    }
    let week = Week {
        start: ss,
        end: es,
        logged_days: logged,
        balance: nutrients::round_to(balance, 1),
        weekly_budget,
        eaten: nutrients::round_to(eaten_w, 1),
        weekly_remaining: weekly_budget.map(|w| nutrients::round_to(w - eaten_w, 1)),
    };
    let weight = weights::on_date(c, user_id, &d)?;
    let streak = streak(c, user_id, date)?;
    let times: Vec<&str> = entries.iter().filter_map(|e| e.time.as_deref()).collect();
    let timing = Timing {
        occasions: occasions(&entries),
        usual: usual_times(c, user_id, date)?,
        first: times.iter().min().map(|t| t.to_string()),
        last: times.iter().max().map(|t| t.to_string()),
        untimed: entries.iter().filter(|e| e.time.is_none()).count(),
    };
    Ok(Diary {
        date: d,
        user_id,
        entries,
        exercises,
        totals: totals.rounded(),
        meals: meals.into_iter().map(|(k, v)| (k, v.rounded())).collect(),
        targets: goals::targets(goal.as_ref()),
        goal,
        budget,
        week,
        weight,
        coverage,
        streak,
        timing,
    })
}

pub fn entry_owner(c: &Connection, id: i64) -> AppResult<i64> {
    c.query_row("SELECT user_id FROM entries WHERE id = ?", [id], |r| {
        r.get(0)
    })
    .optional()?
    .ok_or_else(|| not_found(format!("entry {id} not found")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_quantities() {
        assert_eq!(
            display_quantity(true, 1.25, "serving", Some(1.25), Some("serving")),
            "1.25 servings"
        );
        assert_eq!(
            display_quantity(true, 1.0, "serving", Some(1.0), Some("serving")),
            "1 serving"
        );
        assert_eq!(
            display_quantity(true, 2.0, "cup", Some(480.0), Some("ml")),
            "2 cup (480 ml)"
        );
        assert_eq!(
            display_quantity(true, 1.0, "11 pieces", Some(30.0), Some("g")),
            "1 × 11 pieces (30 g)"
        );
        assert_eq!(
            display_quantity(true, 150.0, "g", Some(150.0), Some("g")),
            "150 g"
        );
        assert_eq!(display_quantity(false, 1.0, "", None, None), "");
    }
}
