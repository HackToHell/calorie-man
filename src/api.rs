use crate::db;
use crate::error::{bad, AppResult};
use crate::external::{self, Source};
use crate::nutrients::{self, CATALOG};
use crate::service::diary::{self, CopyInput, EntryInput, EntryPatch, ExerciseInput, MEALS};
use crate::service::foods::{self, FoodInput, SearchOpts};
use crate::service::goals::{self, GoalInput};
use crate::service::summary;
use crate::service::users::{self, UserInput};
use crate::service::weights::{self, WeightInput, WeightPatch};
use crate::util::{fmt_date, parse_date};
use crate::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

type R = AppResult<Json<Value>>;

/// JSON body extractor whose errors are the app's usual `{"error": ...}` 400 responses, so API and
/// MCP clients see a readable message ("unknown nutrient key(s): protien ...").
#[derive(axum::extract::FromRequest)]
#[from_request(via(axum::Json), rejection(crate::error::AppError))]
pub struct Body<T>(pub T);

fn ok<T: serde::Serialize>(v: T) -> R {
    Ok(Json(serde_json::to_value(v)?))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/nutrients", get(catalog))
        .route("/users", get(list_users).post(create_user))
        .route(
            "/users/{id}",
            get(get_user).put(update_user).delete(delete_user),
        )
        .route("/users/{id}/goals", get(list_goals).post(set_goal))
        .route("/goals/{id}", axum::routing::delete(delete_goal))
        .route("/users/{id}/energy", get(energy))
        .route("/foods", get(search_foods).post(create_food))
        .route(
            "/foods/{id}",
            get(get_food).put(update_food).delete(delete_food),
        )
        .route("/foods/{id}/restore", post(restore_food))
        .route("/users/{id}/recent-foods", get(recent_foods))
        .route("/external/search", get(ext_search))
        .route("/external/barcode/{code}", get(ext_barcode))
        .route("/external/item/{source}/{id}", get(ext_item))
        .route("/external/import", post(ext_import))
        .route("/users/{id}/diary", get(get_diary))
        .route("/users/{id}/entries", post(add_entry))
        .route("/entries/{id}", put(update_entry).delete(delete_entry))
        .route("/users/{id}/copy", post(copy_entries))
        .route("/users/{id}/exercises", post(add_exercise))
        .route("/exercises/{id}", axum::routing::delete(delete_exercise))
        .route("/users/{id}/weights", get(list_weights).post(add_weight))
        .route("/weights/{id}", put(update_weight).delete(delete_weight))
        .route("/users/{id}/summary", get(get_summary))
        .route("/users/{id}/export.csv", get(export_csv))
        .route("/export.json", get(export_json))
        .route("/backup", get(backup))
        .route("/settings", get(get_settings).put(put_settings))
        .fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "no such API endpoint"})),
            )
        })
}

/// The browser sends the active user in `X-Calorie-Man-User` so shared foods record who created them.
fn acting_user(h: &HeaderMap) -> Option<i64> {
    h.get("x-calorie-man-user")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
}

pub fn user_now(u: &users::User) -> (chrono::NaiveDate, chrono::NaiveTime) {
    let tz = u
        .timezone
        .clone()
        .or_else(|| iana_time_zone::get_timezone().ok())
        .and_then(|t| t.parse::<chrono_tz::Tz>().ok());
    match tz {
        Some(tz) => {
            let n = chrono::Utc::now().with_timezone(&tz);
            (n.date_naive(), n.time())
        }
        None => {
            let n = chrono::Local::now();
            (n.date_naive(), n.time())
        }
    }
}

async fn health(State(s): State<AppState>) -> R {
    let n = s
        .db(|c| Ok(c.query_row("SELECT COUNT(*) FROM users", [], |r| r.get::<_, i64>(0))?))
        .await?;
    ok(json!({"ok": true, "version": env!("CARGO_PKG_VERSION"), "users": n}))
}

async fn catalog() -> R {
    ok(json!({
        "nutrients": CATALOG,
        "meals": MEALS,
        "units": {"g": crate::units::compatible("g"), "ml": crate::units::compatible("ml"), "serving": ["serving"]},
        "activity_levels": summary::ACTIVITY_LEVELS,
    }))
}

// ----- users

async fn list_users(State(s): State<AppState>) -> R {
    ok(s.db(|c| users::list(c)).await?)
}

async fn create_user(
    State(s): State<AppState>,
    Body(inp): Body<UserInput>,
) -> AppResult<(StatusCode, Json<Value>)> {
    let u = s.db(move |c| users::create(c, inp)).await?;
    Ok((StatusCode::CREATED, Json(serde_json::to_value(u)?)))
}

async fn get_user(State(s): State<AppState>, Path(id): Path<i64>) -> R {
    ok(s.db(move |c| users::get(c, id)).await?)
}

async fn update_user(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Body(inp): Body<UserInput>,
) -> R {
    ok(s.db(move |c| users::update(c, id, inp)).await?)
}

async fn delete_user(State(s): State<AppState>, Path(id): Path<i64>) -> R {
    s.db(move |c| users::delete(c, id)).await?;
    ok(json!({"deleted": id}))
}

// ----- goals

async fn list_goals(State(s): State<AppState>, Path(id): Path<i64>) -> R {
    ok(s.db(move |c| {
        users::get(c, id)?;
        goals::list(c, id)
    })
    .await?)
}

async fn set_goal(State(s): State<AppState>, Path(id): Path<i64>, Body(inp): Body<GoalInput>) -> R {
    ok(s.db(move |c| {
        let u = users::get(c, id)?;
        goals::set(c, id, inp, user_now(&u).0)
    })
    .await?)
}

async fn delete_goal(State(s): State<AppState>, Path(id): Path<i64>) -> R {
    s.db(move |c| goals::delete(c, id)).await?;
    ok(json!({"deleted": id}))
}

async fn energy(State(s): State<AppState>, Path(id): Path<i64>) -> R {
    ok(s.db(move |c| {
        let u = users::get(c, id)?;
        summary::energy(c, &u, user_now(&u).0)
    })
    .await?)
}

// ----- foods

#[derive(Deserialize)]
struct FoodQuery {
    q: Option<String>,
    limit: Option<usize>,
    kind: Option<String>,
    user_id: Option<i64>,
    archived: Option<bool>,
}

async fn search_foods(State(s): State<AppState>, Query(q): Query<FoodQuery>) -> R {
    ok(s.db(move |c| {
        foods::search(
            c,
            SearchOpts {
                q: q.q.as_deref().unwrap_or(""),
                kind: q.kind.as_deref().filter(|k| !k.is_empty()),
                limit: q.limit.unwrap_or(30),
                user_id: q.user_id,
                include_archived: q.archived.unwrap_or(false),
            },
        )
    })
    .await?)
}

async fn create_food(
    State(s): State<AppState>,
    h: HeaderMap,
    Body(inp): Body<FoodInput>,
) -> AppResult<(StatusCode, Json<Value>)> {
    let by = acting_user(&h);
    let saved = s.db(move |c| foods::create(c, inp, by)).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"food": saved.food, "warnings": saved.warnings})),
    ))
}

async fn get_food(State(s): State<AppState>, Path(id): Path<i64>) -> R {
    ok(s.db(move |c| foods::get(c, id)).await?)
}

#[derive(Deserialize)]
struct PropagateQ {
    propagate: Option<bool>,
}

async fn update_food(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<PropagateQ>,
    Body(inp): Body<FoodInput>,
) -> R {
    let saved = s
        .db(move |c| foods::update(c, id, inp, q.propagate.unwrap_or(true)))
        .await?;
    ok(
        json!({"food": saved.food, "warnings": saved.warnings, "entries_updated": saved.entries_updated}),
    )
}

async fn delete_food(State(s): State<AppState>, Path(id): Path<i64>) -> R {
    let archived = s.db(move |c| foods::delete(c, id)).await?;
    ok(json!({"id": id, "archived": archived, "deleted": !archived}))
}

async fn restore_food(State(s): State<AppState>, Path(id): Path<i64>) -> R {
    ok(s.db(move |c| foods::set_archived(c, id, false)).await?)
}

#[derive(Deserialize)]
struct LimitQ {
    limit: Option<usize>,
}

async fn recent_foods(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<LimitQ>,
) -> R {
    ok(
        s.db(move |c| foods::recent(c, id, q.limit.unwrap_or(20).clamp(1, 100)))
            .await?,
    )
}

// ----- external

#[derive(Deserialize)]
struct ExtQ {
    q: Option<String>,
    source: Option<String>,
    limit: Option<usize>,
    usda_kind: Option<String>,
}

async fn ext_search(State(s): State<AppState>, Query(q): Query<ExtQ>) -> R {
    let src = Source::parse(q.source.as_deref())?;
    ok(external::search(
        &s,
        q.q.as_deref().unwrap_or(""),
        src,
        q.limit.unwrap_or(15),
        q.usda_kind.as_deref().unwrap_or("all"),
    )
    .await?)
}

async fn ext_barcode(State(s): State<AppState>, Path(code): Path<String>) -> R {
    ok(external::barcode(&s, &code).await?)
}

async fn ext_item(State(s): State<AppState>, Path((source, id)): Path<(String, String)>) -> R {
    ok(external::fetch(&s, &source, &id).await?)
}

#[derive(Deserialize)]
struct ImportReq {
    source: String,
    id: String,
    aliases: Option<String>,
    name: Option<String>,
}

async fn ext_import(State(s): State<AppState>, h: HeaderMap, Body(r): Body<ImportReq>) -> R {
    ok(external::import(&s, &r.source, &r.id, acting_user(&h), r.aliases, r.name).await?)
}

// ----- diary

#[derive(Deserialize)]
struct DateQ {
    date: Option<String>,
}

async fn get_diary(State(s): State<AppState>, Path(id): Path<i64>, Query(q): Query<DateQ>) -> R {
    ok(s.db(move |c| {
        let u = users::get(c, id)?;
        let d = crate::util::resolve_date(q.date.as_deref(), user_now(&u).0)?;
        diary::diary(c, id, d)
    })
    .await?)
}

async fn add_entry(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Body(inp): Body<EntryInput>,
) -> AppResult<(StatusCode, Json<Value>)> {
    let e = s
        .db(move |c| {
            let u = users::get(c, id)?;
            let (today, hour) = user_now(&u);
            diary::add_entry(c, id, inp, today, hour)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(serde_json::to_value(e)?)))
}

async fn update_entry(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Body(p): Body<EntryPatch>,
) -> R {
    ok(s.db(move |c| diary::update_entry(c, id, p)).await?)
}

async fn delete_entry(State(s): State<AppState>, Path(id): Path<i64>) -> R {
    ok(s.db(move |c| diary::delete_entry(c, id)).await?)
}

async fn copy_entries(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Body(inp): Body<CopyInput>,
) -> R {
    ok(s.db(move |c| {
        users::get(c, id)?;
        diary::copy_entries(c, id, inp)
    })
    .await?)
}

async fn add_exercise(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Body(inp): Body<ExerciseInput>,
) -> AppResult<(StatusCode, Json<Value>)> {
    let e = s
        .db(move |c| {
            let u = users::get(c, id)?;
            diary::add_exercise(c, id, inp, user_now(&u).0)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(serde_json::to_value(e)?)))
}

async fn delete_exercise(State(s): State<AppState>, Path(id): Path<i64>) -> R {
    s.db(move |c| diary::delete_exercise(c, id)).await?;
    ok(json!({"deleted": id}))
}

// ----- weights

#[derive(Deserialize)]
struct RangeQ {
    from: Option<String>,
    to: Option<String>,
}

async fn list_weights(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<RangeQ>,
) -> R {
    ok(s.db(move |c| {
        users::get(c, id)?;
        if let Some(f) = &q.from {
            parse_date(f)?;
        }
        if let Some(t) = &q.to {
            parse_date(t)?;
        }
        let all = weights::list(c, id, None, q.to.as_deref())?;
        let tr = weights::trend(&all);
        let from = q.from.clone().unwrap_or_default();
        let out: Vec<Value> = all
            .iter()
            .zip(tr.iter())
            .filter(|(w, _)| w.date >= from)
            .map(|(w, t)| {
                let mut v = serde_json::to_value(w).unwrap();
                v["trend_kg"] = json!(t.2);
                v
            })
            .collect();
        Ok(out)
    })
    .await?)
}

async fn add_weight(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Body(inp): Body<WeightInput>,
) -> AppResult<(StatusCode, Json<Value>)> {
    let w = s
        .db(move |c| {
            let u = users::get(c, id)?;
            weights::upsert(c, id, &u.weight_unit, inp, user_now(&u).0)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(serde_json::to_value(w)?)))
}

async fn update_weight(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Body(p): Body<WeightPatch>,
) -> R {
    ok(s.db(move |c| {
        let w = weights::get(c, id)?;
        let u = users::get(c, w.user_id)?;
        weights::update(c, id, &u.weight_unit, p)
    })
    .await?)
}

async fn delete_weight(State(s): State<AppState>, Path(id): Path<i64>) -> R {
    ok(s.db(move |c| weights::delete(c, id)).await?)
}

// ----- summary & export

async fn get_summary(State(s): State<AppState>, Path(id): Path<i64>, Query(q): Query<RangeQ>) -> R {
    ok(s.db(move |c| {
        let u = users::get(c, id)?;
        let today = user_now(&u).0;
        let to = match &q.to {
            Some(t) => parse_date(t)?,
            None => today,
        };
        let from = match &q.from {
            Some(f) => parse_date(f)?,
            None => to - chrono::Duration::days(29),
        };
        summary::summary(c, &u, from, to, today)
    })
    .await?)
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

async fn export_csv(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<RangeQ>,
) -> AppResult<Response> {
    let (name, body) = s
        .db(move |c| {
            let u = users::get(c, id)?;
            let from = q.from.clone().unwrap_or_else(|| "0000-01-01".into());
            let to = q.to.clone().unwrap_or_else(|| "9999-12-31".into());
            let mut st = c.prepare(&format!(
                "SELECT date, meal, name, coalesce(brand, ''), quantity, unit, coalesce(note, ''), {} FROM entries
                 WHERE user_id = ? AND date BETWEEN ? AND ? ORDER BY date, id",
                nutrients::columns("")
            ))?;
            let mut out = String::from("date,meal,food,brand,quantity,unit,note");
            for n in CATALOG {
                out.push_str(&format!(",{}_{}", n.key, n.unit.replace('µ', "u")));
            }
            out.push('\n');
            let mut rows = st.query(rusqlite::params![id, from, to])?;
            while let Some(r) = rows.next()? {
                let mut line = vec![];
                for i in 0..4 {
                    line.push(csv_cell(&r.get::<_, String>(i)?));
                }
                line.push(foods::fmt_num(r.get::<_, f64>(4)?));
                line.push(csv_cell(&r.get::<_, String>(5)?));
                line.push(csv_cell(&r.get::<_, String>(6)?));
                for i in 0..CATALOG.len() {
                    line.push(r.get::<_, Option<f64>>(7 + i)?.map(|v| format!("{}", nutrients::round_to(v, 3))).unwrap_or_default());
                }
                out.push_str(&line.join(","));
                out.push('\n');
            }
            Ok((u.name, out))
        })
        .await?;
    let fname = format!(
        "calorie-man-{}-{}.csv",
        name.replace(|c: char| !c.is_alphanumeric(), "_"),
        fmt_date(chrono::Local::now().date_naive())
    );
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{fname}\""),
            ),
        ],
        body,
    )
        .into_response())
}

async fn export_json(State(s): State<AppState>) -> AppResult<Response> {
    let v = s
        .db(|c| {
            let us = users::list(c)?;
            let all_ids: Vec<i64> = {
                let mut st = c.prepare("SELECT id FROM foods ORDER BY id")?;
                let rows = st.query_map([], |r| r.get(0))?;
                rows.collect::<Result<_, _>>()?
            };
            let mut foods_all = vec![];
            for id in all_ids {
                foods_all.push(foods::get(c, id)?);
            }
            let mut per_user = vec![];
            for u in &us {
                let entries: Vec<diary::Entry> = {
                    let mut st = c.prepare("SELECT DISTINCT date FROM entries WHERE user_id = ? ORDER BY date")?;
                    let dates: Vec<String> = st.query_map([u.id], |r| r.get(0))?.collect::<Result<_, _>>()?;
                    let mut v = vec![];
                    for d in dates {
                        v.extend(diary::entries_for(c, u.id, &d)?);
                    }
                    v
                };
                per_user.push(json!({
                    "user": u,
                    "goals": goals::list(c, u.id)?,
                    "weights": weights::list(c, u.id, None, None)?,
                    "exercises": diary::exercises_between(c, u.id, "0000-01-01", "9999-12-31")?,
                    "entries": entries,
                }));
            }
            Ok(json!({"exported_at": db::now_iso(), "version": env!("CARGO_PKG_VERSION"), "foods": foods_all, "users": per_user}))
        })
        .await?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/json".to_string()),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"calorie-man-export.json\"".to_string(),
            ),
        ],
        serde_json::to_string_pretty(&v)?,
    )
        .into_response())
}

async fn backup(State(s): State<AppState>) -> AppResult<Response> {
    let path = std::env::temp_dir().join(format!(
        "calorie-man-backup-{}-{}.db",
        std::process::id(),
        chrono::Utc::now().timestamp_millis()
    ));
    let p2 = path.clone();
    s.db(move |c| {
        c.execute("VACUUM INTO ?", [p2.to_string_lossy().to_string()])?;
        Ok(())
    })
    .await?;
    let bytes = tokio::fs::read(&path).await.map_err(db::internal)?;
    let _ = tokio::fs::remove_file(&path).await;
    let fname = format!(
        "calorie-man-backup-{}.db",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    Response::builder()
        .header(header::CONTENT_TYPE, "application/vnd.sqlite3")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{fname}\""),
        )
        .body(axum::body::Body::from(bytes))
        .map_err(db::internal)
}

// ----- settings

async fn get_settings(State(s): State<AppState>) -> R {
    let env_key = s.cfg.usda_api_key.is_some();
    let offline = s.cfg.offline;
    ok(s.db(move |c| {
        let k = db::get_setting(c, "usda_api_key")?;
        Ok(json!({
            "usda_api_key_set": k.is_some(),
            "usda_api_key_hint": k.as_ref().map(|k| format!("…{}", &k[k.len().saturating_sub(4)..])),
            "usda_key_source": if k.is_some() { "settings" } else if env_key { "environment" } else { "DEMO_KEY" },
            "offline": offline,
        }))
    })
    .await?)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsIn {
    usda_api_key: Option<String>,
}

async fn put_settings(State(s): State<AppState>, Body(inp): Body<SettingsIn>) -> R {
    if let Some(k) = &inp.usda_api_key {
        if k.len() > 200
            || k.chars()
                .any(|c| c.is_whitespace() && !k.trim().is_empty() && c != ' ')
        {
            return Err(bad("that does not look like an API key"));
        }
    }
    s.db(move |c| db::set_setting(c, "usda_api_key", inp.usda_api_key.as_deref()))
        .await?;
    get_settings(State(s)).await
}
