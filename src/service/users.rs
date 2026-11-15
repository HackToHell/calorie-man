use crate::error::{bad, not_found, AppResult};
use crate::util::{clean_opt, double_option, parse_date};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub sex: Option<String>,
    pub birth_date: Option<String>,
    pub height_cm: Option<f64>,
    pub activity_level: Option<f64>,
    pub weight_unit: String,
    pub timezone: Option<String>,
    pub goal_weight_kg: Option<f64>,
    pub created_at: String,
}

const COLS: &str =
    "id, name, color, sex, birth_date, height_cm, activity_level, weight_unit, timezone, goal_weight_kg, created_at";

fn from_row(r: &Row) -> rusqlite::Result<User> {
    Ok(User {
        id: r.get(0)?,
        name: r.get(1)?,
        color: r.get(2)?,
        sex: r.get(3)?,
        birth_date: r.get(4)?,
        height_cm: r.get(5)?,
        activity_level: r.get(6)?,
        weight_unit: r.get(7)?,
        timezone: r.get(8)?,
        goal_weight_kg: r.get(9)?,
        created_at: r.get(10)?,
    })
}

/// Profile fields. Missing fields are left unchanged on update; `null` clears them.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserInput {
    pub name: Option<String>,
    pub color: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub sex: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub birth_date: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub height_cm: Option<Option<f64>>,
    #[serde(default, deserialize_with = "double_option")]
    pub activity_level: Option<Option<f64>>,
    pub weight_unit: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub timezone: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub goal_weight_kg: Option<Option<f64>>,
}

pub const PALETTE: &[&str] = &[
    "#7a4a8c", "#c0663a", "#2f8a7e", "#5466b0", "#a8507a", "#7d7330", "#3d7ea6", "#9a5b3a",
];

pub fn list(c: &Connection) -> AppResult<Vec<User>> {
    let mut st = c.prepare(&format!("SELECT {COLS} FROM users ORDER BY id"))?;
    let rows = st.query_map([], from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get(c: &Connection, id: i64) -> AppResult<User> {
    c.query_row(
        &format!("SELECT {COLS} FROM users WHERE id = ?"),
        [id],
        from_row,
    )
    .optional()?
    .ok_or_else(|| not_found(format!("user {id} not found")))
}

/// Find a user by id, exact name (case-insensitive), or a unique name prefix.
pub fn resolve(c: &Connection, spec: &str) -> AppResult<User> {
    let spec = spec.trim();
    if let Ok(id) = spec.parse::<i64>() {
        return get(c, id);
    }
    let all = list(c)?;
    if let Some(u) = all.iter().find(|u| u.name.eq_ignore_ascii_case(spec)) {
        return Ok(u.clone());
    }
    let lower = spec.to_lowercase();
    let matches: Vec<_> = all
        .iter()
        .filter(|u| u.name.to_lowercase().starts_with(&lower))
        .collect();
    match matches.len() {
        1 => Ok(matches[0].clone()),
        _ => Err(not_found(format!(
            "no user named '{spec}'. Users: {}",
            all.iter()
                .map(|u| format!("{} (id {})", u.name, u.id))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn validate(u: &User) -> AppResult<()> {
    let name = u.name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err(bad("name must be 1-40 characters"));
    }
    let c = &u.color;
    if !(c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())) {
        return Err(bad("color must look like #2f855a"));
    }
    if let Some(s) = &u.sex {
        if s != "male" && s != "female" {
            return Err(bad(
                "sex must be 'male' or 'female' (used for BMR estimates)",
            ));
        }
    }
    if let Some(b) = &u.birth_date {
        let d = parse_date(b)?;
        if d >= chrono::Local::now().date_naive() {
            return Err(bad("birth_date must be in the past"));
        }
    }
    if let Some(h) = u.height_cm {
        if !(50.0..=272.0).contains(&h) {
            return Err(bad("height_cm must be between 50 and 272"));
        }
    }
    if let Some(a) = u.activity_level {
        if !(1.0..=2.5).contains(&a) {
            return Err(bad(
                "activity_level must be between 1.0 and 2.5 (1.2 sedentary ... 1.9 very active)",
            ));
        }
    }
    if u.weight_unit != "kg" && u.weight_unit != "lb" {
        return Err(bad("weight_unit must be 'kg' or 'lb'"));
    }
    if let Some(tz) = &u.timezone {
        if tz.parse::<chrono_tz::Tz>().is_err() {
            return Err(bad(format!(
                "unknown time zone '{tz}', use an IANA name like America/Los_Angeles"
            )));
        }
    }
    if let Some(g) = u.goal_weight_kg {
        if !(20.0..=400.0).contains(&g) {
            return Err(bad("goal_weight_kg must be between 20 and 400"));
        }
    }
    Ok(())
}

fn apply(u: &mut User, inp: UserInput) {
    if let Some(n) = inp.name {
        u.name = n.trim().to_string();
    }
    if let Some(c) = inp.color {
        u.color = c.trim().to_lowercase();
    }
    if let Some(v) = inp.sex {
        u.sex = clean_opt(v).map(|s| s.to_lowercase());
    }
    if let Some(v) = inp.birth_date {
        u.birth_date = clean_opt(v);
    }
    if let Some(v) = inp.height_cm {
        u.height_cm = v;
    }
    if let Some(v) = inp.activity_level {
        u.activity_level = v;
    }
    if let Some(v) = inp.weight_unit {
        u.weight_unit = v.trim().to_lowercase();
    }
    if let Some(v) = inp.timezone {
        u.timezone = clean_opt(v);
    }
    if let Some(v) = inp.goal_weight_kg {
        u.goal_weight_kg = v;
    }
}

pub fn create(c: &Connection, inp: UserInput) -> AppResult<User> {
    let count: i64 = c.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
    let mut u = User {
        id: 0,
        name: String::new(),
        color: PALETTE[(count as usize) % PALETTE.len()].to_string(),
        sex: None,
        birth_date: None,
        height_cm: None,
        activity_level: None,
        weight_unit: "kg".into(),
        timezone: None,
        goal_weight_kg: None,
        created_at: String::new(),
    };
    if inp.name.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return Err(bad("name is required"));
    }
    apply(&mut u, inp);
    validate(&u)?;
    if c.query_row(
        "SELECT 1 FROM users WHERE name = ? COLLATE NOCASE",
        [&u.name],
        |_| Ok(()),
    )
    .optional()?
    .is_some()
    {
        return Err(bad(format!("a user named '{}' already exists", u.name)));
    }
    c.execute(
        "INSERT INTO users (name, color, sex, birth_date, height_cm, activity_level, weight_unit, timezone, goal_weight_kg)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![u.name, u.color, u.sex, u.birth_date, u.height_cm, u.activity_level, u.weight_unit, u.timezone, u.goal_weight_kg],
    )?;
    get(c, c.last_insert_rowid())
}

pub fn update(c: &Connection, id: i64, inp: UserInput) -> AppResult<User> {
    let mut u = get(c, id)?;
    apply(&mut u, inp);
    validate(&u)?;
    if c.query_row(
        "SELECT 1 FROM users WHERE name = ? COLLATE NOCASE AND id != ?",
        params![u.name, id],
        |_| Ok(()),
    )
    .optional()?
    .is_some()
    {
        return Err(bad(format!("a user named '{}' already exists", u.name)));
    }
    c.execute(
        "UPDATE users SET name=?1, color=?2, sex=?3, birth_date=?4, height_cm=?5, activity_level=?6, weight_unit=?7,
         timezone=?8, goal_weight_kg=?9 WHERE id=?10",
        params![u.name, u.color, u.sex, u.birth_date, u.height_cm, u.activity_level, u.weight_unit, u.timezone, u.goal_weight_kg, id],
    )?;
    get(c, id)
}

pub fn delete(c: &Connection, id: i64) -> AppResult<()> {
    get(c, id)?;
    c.execute("DELETE FROM users WHERE id = ?", [id])?;
    Ok(())
}

pub fn age_on(u: &User, day: chrono::NaiveDate) -> Option<i32> {
    use chrono::Datelike;
    let b = parse_date(u.birth_date.as_deref()?).ok()?;
    let mut age = day.year() - b.year();
    if (day.month(), day.day()) < (b.month(), b.day()) {
        age -= 1;
    }
    Some(age)
}

pub fn today(u: &User) -> chrono::NaiveDate {
    crate::util::today_in(u.timezone.as_deref())
}
