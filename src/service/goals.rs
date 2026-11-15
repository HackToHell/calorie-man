use crate::error::{bad, not_found, AppResult};
use crate::nutrients::{resolve_key, CATALOG};
use crate::util::{double_option, fmt_date, parse_date};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A goal set applies from `effective_from` until the next goal set. Past days keep the goal that was
/// in effect then.
#[derive(Debug, Clone, Serialize)]
pub struct Goal {
    pub id: i64,
    pub user_id: i64,
    pub effective_from: String,
    pub calories: Option<f64>,
    pub protein: Option<f64>,
    pub carbs: Option<f64>,
    pub fat: Option<f64>,
    pub fiber: Option<f64>,
    pub exercise_adds_to_budget: bool,
    /// Overrides of the default daily values for other nutrients, by catalog key.
    pub micros: BTreeMap<String, f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalInput {
    pub effective_from: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub calories: Option<Option<f64>>,
    #[serde(default, deserialize_with = "double_option")]
    pub protein: Option<Option<f64>>,
    #[serde(default, deserialize_with = "double_option")]
    pub carbs: Option<Option<f64>>,
    #[serde(default, deserialize_with = "double_option")]
    pub fat: Option<Option<f64>>,
    #[serde(default, deserialize_with = "double_option")]
    pub fiber: Option<Option<f64>>,
    pub exercise_adds_to_budget: Option<bool>,
    pub micros: Option<BTreeMap<String, Option<f64>>>,
}

const COLS: &str = "id, user_id, effective_from, calories, protein, carbs, fat, fiber, exercise_adds_to_budget, micros";

fn from_row(r: &Row) -> rusqlite::Result<Goal> {
    let micros: String = r.get(9)?;
    Ok(Goal {
        id: r.get(0)?,
        user_id: r.get(1)?,
        effective_from: r.get(2)?,
        calories: r.get(3)?,
        protein: r.get(4)?,
        carbs: r.get(5)?,
        fat: r.get(6)?,
        fiber: r.get(7)?,
        exercise_adds_to_budget: r.get::<_, i64>(8)? != 0,
        micros: serde_json::from_str(&micros).unwrap_or_default(),
    })
}

/// All goal sets for a user, oldest first.
pub fn list(c: &Connection, user_id: i64) -> AppResult<Vec<Goal>> {
    let mut st = c.prepare(&format!(
        "SELECT {COLS} FROM goals WHERE user_id = ? ORDER BY effective_from"
    ))?;
    let rows = st.query_map([user_id], from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The goal in effect on `date`: the latest one starting on or before it. Days before the first goal
/// use the first goal, so early logs still get a budget.
pub fn pick<'a>(goals: &'a [Goal], date: &str) -> Option<&'a Goal> {
    goals
        .iter()
        .rev()
        .find(|g| g.effective_from.as_str() <= date)
        .or_else(|| goals.first())
}

pub fn for_date(c: &Connection, user_id: i64, date: &str) -> AppResult<Option<Goal>> {
    Ok(pick(&list(c, user_id)?, date).cloned())
}

fn check(v: Option<f64>, what: &str, max: f64) -> AppResult<Option<f64>> {
    if let Some(x) = v {
        if !x.is_finite() || x < 0.0 || x > max {
            return Err(bad(format!("{what} goal must be between 0 and {max}")));
        }
    }
    Ok(v)
}

/// Create or replace the goal starting on `effective_from` (default `today`). Fields that are not given
/// are copied from the goal in effect on that date; explicit nulls clear them.
pub fn set(
    c: &Connection,
    user_id: i64,
    inp: GoalInput,
    today: chrono::NaiveDate,
) -> AppResult<Goal> {
    let from = match inp.effective_from.as_deref() {
        Some(s) => crate::util::resolve_date(Some(s), today)?,
        None => today,
    };
    let from_s = fmt_date(from);
    let base = for_date(c, user_id, &from_s)?;
    let pickv = |given: Option<Option<f64>>, old: Option<f64>| given.unwrap_or(old);
    let calories = check(
        pickv(inp.calories, base.as_ref().and_then(|b| b.calories)),
        "calorie",
        20000.0,
    )?;
    let protein = check(
        pickv(inp.protein, base.as_ref().and_then(|b| b.protein)),
        "protein",
        1000.0,
    )?;
    let carbs = check(
        pickv(inp.carbs, base.as_ref().and_then(|b| b.carbs)),
        "carbs",
        2000.0,
    )?;
    let fat = check(
        pickv(inp.fat, base.as_ref().and_then(|b| b.fat)),
        "fat",
        1000.0,
    )?;
    let fiber = check(
        pickv(inp.fiber, base.as_ref().and_then(|b| b.fiber)),
        "fiber",
        500.0,
    )?;
    let ex = inp.exercise_adds_to_budget.unwrap_or(
        base.as_ref()
            .map(|b| b.exercise_adds_to_budget)
            .unwrap_or(false),
    );
    let mut micros = base.as_ref().map(|b| b.micros.clone()).unwrap_or_default();
    if let Some(m) = inp.micros {
        for (k, v) in m {
            let i =
                resolve_key(&k).ok_or_else(|| bad(format!("unknown nutrient '{k}' in micros")))?;
            let key = CATALOG[i].key.to_string();
            match v {
                Some(x) if x.is_finite() && x >= 0.0 => {
                    micros.insert(key, x);
                }
                Some(_) => {
                    return Err(bad(format!("target for {k} must be a non-negative number")))
                }
                None => {
                    micros.remove(&key);
                }
            }
        }
    }
    let micros_json = serde_json::to_string(&micros)?;
    c.execute(
        "INSERT INTO goals (user_id, effective_from, calories, protein, carbs, fat, fiber, exercise_adds_to_budget, micros)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(user_id, effective_from) DO UPDATE SET calories=excluded.calories, protein=excluded.protein,
           carbs=excluded.carbs, fat=excluded.fat, fiber=excluded.fiber,
           exercise_adds_to_budget=excluded.exercise_adds_to_budget, micros=excluded.micros",
        params![user_id, from_s, calories, protein, carbs, fat, fiber, ex as i64, micros_json],
    )?;
    let g = c.query_row(
        &format!("SELECT {COLS} FROM goals WHERE user_id = ? AND effective_from = ?"),
        params![user_id, from_s],
        from_row,
    )?;
    Ok(g)
}

pub fn delete(c: &Connection, id: i64) -> AppResult<()> {
    let n = c.execute("DELETE FROM goals WHERE id = ?", [id])?;
    if n == 0 {
        return Err(not_found(format!("goal {id} not found")));
    }
    Ok(())
}

pub fn get(c: &Connection, id: i64) -> AppResult<Goal> {
    c.query_row(
        &format!("SELECT {COLS} FROM goals WHERE id = ?"),
        [id],
        from_row,
    )
    .optional()?
    .ok_or_else(|| not_found(format!("goal {id} not found")))
}

#[derive(Debug, Clone, Serialize)]
pub struct Target {
    pub key: &'static str,
    pub target: f64,
    /// true when the target is an upper limit (stay under it).
    pub limit: bool,
    /// "goal" when set by the user, "dv" when it is the FDA daily value.
    pub from: &'static str,
}

/// Targets for every nutrient that has one: macros from the goal, the rest from goal overrides or the
/// FDA daily values.
pub fn targets(goal: Option<&Goal>) -> Vec<Target> {
    let mut out = vec![];
    for n in CATALOG {
        let from_goal = goal.and_then(|g| match n.key {
            "calories" => g.calories,
            "protein" => g.protein,
            "carbs" => g.carbs,
            "fat" => g.fat,
            "fiber" => g.fiber,
            k => g.micros.get(k).copied(),
        });
        if let Some(t) = from_goal {
            out.push(Target {
                key: n.key,
                target: t,
                limit: n.limit,
                from: "goal",
            });
        } else if n.key != "calories" {
            if let Some(dv) = n.dv {
                out.push(Target {
                    key: n.key,
                    target: dv,
                    limit: n.limit,
                    from: "dv",
                });
            }
        }
    }
    out
}

pub fn validate_date(s: &str) -> AppResult<()> {
    parse_date(s).map(|_| ())
}
