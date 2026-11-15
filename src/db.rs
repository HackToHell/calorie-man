use crate::error::{AppError, AppResult};
use crate::nutrients::CATALOG;
use rusqlite::Connection;

#[cfg(not(target_arch = "wasm32"))]
pub type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;
/// The browser build has one thread and one in-memory database.
#[cfg(target_arch = "wasm32")]
pub type Pool = std::sync::Arc<std::sync::Mutex<Connection>>;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    color TEXT NOT NULL DEFAULT '#2f855a',
    sex TEXT CHECK (sex IN ('male', 'female')),
    birth_date TEXT,
    height_cm REAL,
    activity_level REAL,
    weight_unit TEXT NOT NULL DEFAULT 'kg' CHECK (weight_unit IN ('kg', 'lb')),
    timezone TEXT,
    goal_weight_kg REAL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE IF NOT EXISTS goals (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    effective_from TEXT NOT NULL,
    calories REAL,
    protein REAL,
    carbs REAL,
    fat REAL,
    fiber REAL,
    exercise_adds_to_budget INTEGER NOT NULL DEFAULT 0,
    micros TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE (user_id, effective_from)
);

CREATE TABLE IF NOT EXISTS foods (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    brand TEXT,
    kind TEXT NOT NULL DEFAULT 'food' CHECK (kind IN ('food', 'recipe')),
    basis_amount REAL NOT NULL DEFAULT 100 CHECK (basis_amount > 0),
    basis_unit TEXT NOT NULL DEFAULT 'g' CHECK (basis_unit IN ('g', 'ml', 'serving')),
    barcode TEXT,
    source TEXT NOT NULL DEFAULT 'manual',
    source_id TEXT,
    aliases TEXT,
    notes TEXT,
    recipe_yield REAL,
    archived INTEGER NOT NULL DEFAULT 0,
    created_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS foods_source_id ON foods(source, source_id) WHERE source_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS foods_barcode ON foods(barcode) WHERE barcode IS NOT NULL;

CREATE TABLE IF NOT EXISTS food_servings (
    id INTEGER PRIMARY KEY,
    food_id INTEGER NOT NULL REFERENCES foods(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    amount REAL NOT NULL CHECK (amount > 0),
    is_default INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS food_servings_food ON food_servings(food_id);

CREATE TABLE IF NOT EXISTS recipe_items (
    id INTEGER PRIMARY KEY,
    recipe_id INTEGER NOT NULL REFERENCES foods(id) ON DELETE CASCADE,
    food_id INTEGER NOT NULL REFERENCES foods(id),
    quantity REAL NOT NULL,
    unit TEXT NOT NULL,
    serving_id INTEGER REFERENCES food_servings(id) ON DELETE SET NULL,
    amount REAL NOT NULL,
    position INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS recipe_items_recipe ON recipe_items(recipe_id);
CREATE INDEX IF NOT EXISTS recipe_items_food ON recipe_items(food_id);

CREATE TABLE IF NOT EXISTS entries (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    meal TEXT NOT NULL DEFAULT 'snacks' CHECK (meal IN ('breakfast', 'lunch', 'dinner', 'snacks')),
    food_id INTEGER REFERENCES foods(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    brand TEXT,
    quantity REAL NOT NULL DEFAULT 1,
    unit TEXT NOT NULL DEFAULT 'serving',
    serving_id INTEGER REFERENCES food_servings(id) ON DELETE SET NULL,
    amount REAL,
    note TEXT,
    source TEXT NOT NULL DEFAULT 'web',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    eaten_at TEXT
);
CREATE INDEX IF NOT EXISTS entries_user_date ON entries(user_id, date);
CREATE INDEX IF NOT EXISTS entries_food ON entries(food_id);

CREATE TABLE IF NOT EXISTS exercises (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    name TEXT NOT NULL,
    calories REAL NOT NULL CHECK (calories >= 0),
    duration_min REAL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX IF NOT EXISTS exercises_user_date ON exercises(user_id, date);

CREATE TABLE IF NOT EXISTS weights (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    weight_kg REAL NOT NULL CHECK (weight_kg > 0),
    body_fat_pct REAL,
    note TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE (user_id, date)
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS external_cache (
    url TEXT PRIMARY KEY,
    body TEXT NOT NULL,
    fetched_at INTEGER NOT NULL
);
"#;

fn init_conn(c: &mut Connection) -> rusqlite::Result<()> {
    c.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000; PRAGMA synchronous = NORMAL;",
    )
}

#[cfg(target_arch = "wasm32")]
pub fn open_memory() -> anyhow::Result<Pool> {
    let mut c = Connection::open_in_memory()?;
    init_conn(&mut c)?;
    migrate(&mut c)?;
    Ok(std::sync::Arc::new(std::sync::Mutex::new(c)))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn open_pool(path: &std::path::Path) -> anyhow::Result<Pool> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    {
        let mut c = Connection::open(path)?;
        c.execute_batch("PRAGMA journal_mode = WAL;")?;
        init_conn(&mut c)?;
        migrate(&mut c)?;
    }
    let mgr = r2d2_sqlite::SqliteConnectionManager::file(path).with_init(init_conn);
    Ok(r2d2::Pool::builder().max_size(8).build(mgr)?)
}

/// Create tables, then add a column for every catalog nutrient that is missing. New nutrients in the
/// catalog therefore migrate themselves.
pub fn migrate(c: &mut Connection) -> rusqlite::Result<()> {
    c.execute_batch(SCHEMA)?;
    for table in ["foods", "entries"] {
        let existing: Vec<String> = {
            let mut st = c.prepare(&format!("PRAGMA table_info({table})"))?;
            let rows = st.query_map([], |r| r.get::<_, String>(1))?;
            rows.collect::<Result<_, _>>()?
        };
        for n in CATALOG {
            let col = format!("n_{}", n.key);
            if !existing.contains(&col) {
                c.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {col} REAL"))?;
            }
        }
    }
    // v2: the time of day each entry was eaten ("HH:MM", the user's local time).
    let has_time: bool = c
        .prepare("PRAGMA table_info(entries)")?
        .query_map([], |r| r.get::<_, String>(1))?
        .filter_map(Result::ok)
        .any(|col| col == "eaten_at");
    if !has_time {
        c.execute_batch("ALTER TABLE entries ADD COLUMN eaten_at TEXT")?;
        backfill_eaten_at(c)?;
    }
    c.execute_batch("PRAGMA user_version = 2;")?;
    Ok(())
}

/// For entries logged on the same local day they belong to, the logging time is a good guess of
/// when they were eaten. Copies and anything back-filled later stay without a time.
fn backfill_eaten_at(c: &Connection) -> rusqlite::Result<()> {
    let rows: Vec<(i64, String, String, Option<String>)> = {
        let mut st = c.prepare(
            "SELECT e.id, e.date, e.created_at, u.timezone FROM entries e JOIN users u ON u.id = e.user_id
             WHERE e.eaten_at IS NULL AND e.source NOT IN ('copy', 'demo')",
        )?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    let server_tz = iana_time_zone::get_timezone().ok();
    for (id, date, created, tz) in rows {
        let Ok(utc) = chrono::DateTime::parse_from_rfc3339(&created) else {
            continue;
        };
        let Some(tz) = tz
            .or(server_tz.clone())
            .and_then(|t| t.parse::<chrono_tz::Tz>().ok())
        else {
            continue;
        };
        let local = utc.with_timezone(&tz);
        if local.format("%Y-%m-%d").to_string() == date {
            c.execute(
                "UPDATE entries SET eaten_at = ? WHERE id = ?",
                rusqlite::params![local.format("%H:%M").to_string(), id],
            )?;
        }
    }
    Ok(())
}

pub fn get_setting(c: &Connection, key: &str) -> AppResult<Option<String>> {
    use rusqlite::OptionalExtension;
    Ok(
        c.query_row("SELECT value FROM settings WHERE key = ?", [key], |r| {
            r.get(0)
        })
        .optional()?,
    )
}

pub fn set_setting(c: &Connection, key: &str, value: Option<&str>) -> AppResult<()> {
    match value {
        Some(v) if !v.trim().is_empty() => {
            c.execute(
                "INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [key, v.trim()],
            )?;
        }
        _ => {
            c.execute("DELETE FROM settings WHERE key = ?", [key])?;
        }
    }
    Ok(())
}

pub fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn internal(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(e.to_string())
}
