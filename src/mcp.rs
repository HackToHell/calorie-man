//! MCP (Model Context Protocol) server over Streamable HTTP at `/mcp`.
//!
//! Stateless: every POST carries one JSON-RPC message (or a batch) and gets a JSON response. No
//! session ids and no server-initiated streams (GET returns 405), which the spec allows. Tools call the
//! same service layer as the REST API.

use crate::api::user_now;
use crate::error::{bad, AppError, AppResult};
use crate::external::{self, Source};
use crate::nutrients::{round_to, Nutrients, CATALOG};
use crate::service::diary::{self, CopyInput, EntryInput, EntryPatch, ExerciseInput};
use crate::service::foods::{self, FoodInput, IngredientInput, SearchOpts, ServingInput};
use crate::service::goals::{self, GoalInput};
use crate::service::summary;
use crate::service::users::{self, User};
use crate::service::weights::{self, WeightInput};
use crate::util::{fmt_date, resolve_date};
use crate::AppState;
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use serde::Deserialize;
use serde_json::{json, Map, Value};

pub const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/mcp",
        post(handle_post)
            .get(method_not_allowed)
            .delete(method_not_allowed),
    )
}

async fn method_not_allowed() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [(header::ALLOW, "POST")],
        "This MCP server is stateless; use POST.",
    )
        .into_response()
}

#[derive(Deserialize, Default, Clone)]
pub struct McpQuery {
    /// Default user for tools when the `user` argument is omitted (e.g. /mcp?user=alex).
    pub user: Option<String>,
}

fn origin_allowed(state: &AppState, h: &HeaderMap) -> bool {
    let Some(origin) = h.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return true; // Non-browser clients do not send Origin.
    };
    if state
        .cfg
        .allowed_origins
        .iter()
        .any(|o| o == "*" || o == origin)
    {
        return true;
    }
    let ohost = origin
        .split("://")
        .nth(1)
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("");
    let host = h
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let bare = |s: &str| {
        s.rsplit_once(':')
            .map(|(a, _)| a.to_string())
            .unwrap_or(s.to_string())
    };
    ohost == host || matches!(bare(ohost).as_str(), "localhost" | "127.0.0.1" | "[::1]")
}

async fn handle_post(
    State(state): State<AppState>,
    Query(q): Query<McpQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !origin_allowed(&state, &headers) {
        return (
            StatusCode::FORBIDDEN,
            "Origin not allowed (set CALORIE_MAN_ALLOWED_ORIGINS)",
        )
            .into_response();
    }
    let msg: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                axum::Json(rpc_error(Value::Null, -32700, &format!("parse error: {e}"))),
            )
                .into_response()
        }
    };
    match msg {
        Value::Array(batch) => {
            let mut out = vec![];
            for m in batch {
                if let Some(r) = handle_message(&state, &q, m).await {
                    out.push(r);
                }
            }
            if out.is_empty() {
                StatusCode::ACCEPTED.into_response()
            } else {
                axum::Json(Value::Array(out)).into_response()
            }
        }
        m => match handle_message(&state, &q, m).await {
            Some(r) => axum::Json(r).into_response(),
            None => StatusCode::ACCEPTED.into_response(),
        },
    }
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// Handle one JSON-RPC message. Notifications and client responses produce no reply.
pub async fn handle_message(state: &AppState, q: &McpQuery, m: Value) -> Option<Value> {
    let Some(obj) = m.as_object() else {
        return Some(rpc_error(
            Value::Null,
            -32600,
            "invalid request: expected a JSON object",
        ));
    };
    let method = obj.get("method").and_then(|v| v.as_str());
    let id = obj.get("id").cloned();
    let (Some(method), Some(id)) = (method, id) else {
        if method.is_none() && obj.get("id").is_none() {
            return Some(rpc_error(
                Value::Null,
                -32600,
                "invalid request: missing method",
            ));
        }
        return None; // notification or a response to us
    };
    let params = obj.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => {
            let requested = params["protocolVersion"]
                .as_str()
                .unwrap_or(PROTOCOL_VERSIONS[0]);
            let version = if PROTOCOL_VERSIONS.contains(&requested) {
                requested
            } else {
                PROTOCOL_VERSIONS[0]
            };
            rpc_result(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "calorie-man", "title": "Calorie Man: calorie & nutrition tracker", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": instructions(state, q).await,
                }),
            )
        }
        "ping" => rpc_result(id, json!({})),
        "tools/list" => rpc_result(id, json!({"tools": tool_defs()})),
        "prompts/list" => rpc_result(id, json!({"prompts": []})),
        "resources/list" => rpc_result(id, json!({"resources": []})),
        "resources/templates/list" => rpc_result(id, json!({"resourceTemplates": []})),
        "logging/setLevel" => rpc_result(id, json!({})),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or("");
            if !tool_defs().iter().any(|t| t["name"] == name) {
                return Some(rpc_error(id, -32602, &format!("unknown tool '{name}'")));
            }
            let args = match &params["arguments"] {
                Value::Null => Value::Object(Map::new()),
                v => v.clone(),
            };
            let res = call_tool(state, q, name, args).await;
            rpc_result(
                id,
                match res {
                    Ok(v) => {
                        json!({"content": [{"type": "text", "text": serde_json::to_string(&v).unwrap_or_default()}], "isError": false})
                    }
                    Err(e) => {
                        json!({"content": [{"type": "text", "text": format!("Error: {e}")}], "isError": true})
                    }
                },
            )
        }
        other => rpc_error(id, -32601, &format!("method not found: {other}")),
    })
}

async fn instructions(state: &AppState, q: &McpQuery) -> String {
    let users = state.db(|c| users::list(c)).await.unwrap_or_default();
    let names = users
        .iter()
        .map(|u| format!("{} (id {})", u.name, u.id))
        .collect::<Vec<_>>()
        .join(", ");
    let default = q
        .user
        .clone()
        .or_else(|| (users.len() == 1).then(|| users[0].name.clone()));
    format!(
        "Calorie Man is a household calorie and nutrition tracker. The food library (foods and recipes) is shared by all \
users; diaries, goals and weights are per user. Users: {names}. {}\n\n\
Logging food:\n\
1. search_foods first (the shared library; 'costco' also matches 'kirkland').\n\
2. If it is not there, search_online_foods (USDA FoodData Central + Open Food Facts) and import_online_food the \
best match. Prefer branded USDA items for packaged US products (Costco's brand is Kirkland Signature) and \
USDA generic items (SR Legacy/Foundation) for whole foods, since those have full vitamin data. Add aliases \
the user would say (e.g. 'costco milk') when importing.\n\
3. If nothing online fits, create_food with the label values. Nutrients are per basis_amount of basis_unit \
(default per 100 g; use basis_unit 'serving' for restaurant items). Give every value you know; keys are in \
get_nutrient_catalog (calories kcal, protein/carbs/fat/fiber/sugar g, sodium mg, vitamin_d µg, ...). Add named \
servings (e.g. {{label: 'cup', amount: 240}}) so quantities like '2 cups' work.\n\
4. log_food with a quantity and unit (a serving label, g/oz, ml/cup, or 'serving'). For a meal you cannot \
break down, use quick_add with estimated nutrients. Pass `time` (e.g. '7:40 am') whenever the user says when \
they ate; it powers the meal-timing chart and eating window.\n\
Recipes: create_recipe from existing foods (e.g. 'Morning oats' = oats + milk + banana); both users can log it.\n\
Dates are YYYY-MM-DD or 'today'/'yesterday' in the user's time zone. Meals: breakfast, lunch, dinner, snacks \
(default from the time of day). Never invent nutrition numbers when a real source exists; tell the user when \
values are estimates.",
        match default {
            Some(d) => format!("When no user is given, tools act for {d}."),
            None => "Pass `user` (name or id) to user-specific tools.".into(),
        }
    )
}

// ---------------------------------------------------------------------------------------------------------
// Tool definitions

fn nutrients_schema() -> Value {
    let mut props = Map::new();
    for n in CATALOG {
        props.insert(
            n.key.into(),
            json!({"type": ["number", "null"], "description": format!("{} ({})", n.label, n.unit)}),
        );
    }
    json!({"type": "object", "properties": props, "additionalProperties": false,
           "description": "Nutrient amounts for the basis amount. Omit unknown values (unknown is not zero)."})
}

fn user_prop() -> Value {
    json!({"type": ["string", "integer"], "description": "User name or id. Optional when the server has a default user."})
}

fn date_prop(what: &str) -> Value {
    json!({"type": "string", "description": format!("{what}: YYYY-MM-DD, 'today', 'yesterday' or a day offset like '-2'. Defaults to today.")})
}

fn time_prop() -> Value {
    json!({"type": "string", "description": "When it was eaten, user's local time: \"07:40\", \"7:40 pm\" or \"now\". Pass it whenever the user mentions a time. If omitted, today's entries get the current time when the meal fits the time of day; otherwise the time stays unknown."})
}

fn servings_schema() -> Value {
    json!({"type": "array", "description": "Named servings so the food can be logged as '2 cups' or '1 bar'.",
        "items": {"type": "object", "required": ["label", "amount"], "additionalProperties": false, "properties": {
            "label": {"type": "string", "description": "e.g. 'cup', 'bar', 'slice', 'bottle (591 ml)'"},
            "amount": {"type": "number", "description": "Size in `unit`, or in the basis unit when unit is omitted"},
            "unit": {"type": "string", "description": "Optional unit of `amount`: g, oz, ml, cup, tbsp, ... (must match the basis: weight or volume)"},
            "is_default": {"type": "boolean"}}}})
}

fn ingredients_schema() -> Value {
    json!({"type": "array", "minItems": 1, "items": {"type": "object", "required": ["food_id", "quantity"], "additionalProperties": false,
        "properties": {
            "food_id": {"type": "integer"},
            "quantity": {"type": "number"},
            "unit": {"type": "string", "description": "Serving label or unit (g, oz, ml, cup, serving)"},
            "serving_id": {"type": "integer"}}}})
}

fn tool(
    name: &str,
    title: &str,
    desc: &str,
    props: Value,
    required: &[&str],
    read_only: bool,
) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": desc,
        "inputSchema": {"type": "object", "properties": props, "required": required, "additionalProperties": false},
        "annotations": {"readOnlyHint": read_only, "destructiveHint": false, "idempotentHint": read_only, "openWorldHint": name.contains("online") || name == "lookup_barcode"},
    })
}

pub fn tool_defs() -> Vec<Value> {
    vec![
        tool("list_users", "List users", "List household users with their ids, units, today's date and current goals.", json!({}), &[], true),
        tool("get_nutrient_catalog", "Nutrient catalog", "All nutrient keys with units, daily values and accepted aliases.", json!({}), &[], true),
        tool("search_foods", "Search food library",
            "Search the shared food library (foods and recipes). Every word must match the name, brand, aliases or barcode.",
            json!({"query": {"type": "string"}, "kind": {"type": "string", "enum": ["food", "recipe"]}, "limit": {"type": "integer", "minimum": 1, "maximum": 50}}),
            &["query"], true),
        tool("get_food", "Get food", "Full details of a food or recipe: every nutrient, servings, ingredients.",
            json!({"food_id": {"type": "integer"}}), &["food_id"], true),
        tool("search_online_foods", "Search online food databases",
            "Search USDA FoodData Central and Open Food Facts. Returns candidates with key nutrients; import one with import_online_food. 'costco' is searched as 'kirkland'.",
            json!({"query": {"type": "string"},
                   "source": {"type": "string", "enum": ["all", "usda", "off"], "description": "Default all"},
                   "usda_kind": {"type": "string", "enum": ["all", "generic", "branded"], "description": "generic = whole foods with full vitamin data; branded = packaged products"},
                   "limit": {"type": "integer", "minimum": 1, "maximum": 25}}),
            &["query"], true),
        tool("lookup_barcode", "Look up barcode", "Find a product by UPC/EAN barcode: the library first, then Open Food Facts and USDA.",
            json!({"barcode": {"type": "string"}}), &["barcode"], true),
        tool("import_online_food", "Import online food",
            "Copy an online item into the shared library (returns the existing food if it was imported before).",
            json!({"source": {"type": "string", "enum": ["usda", "off"]}, "id": {"type": "string", "description": "USDA fdcId or OFF barcode"},
                   "aliases": {"type": "string", "description": "Extra search words, e.g. 'costco milk'"},
                   "name": {"type": "string", "description": "Optional nicer name"}}),
            &["source", "id"], false),
        tool("create_food", "Create food",
            "Add a food to the shared library when neither the library nor the online databases have it. Nutrients are per basis_amount of basis_unit (default 100 g).",
            json!({"name": {"type": "string"}, "brand": {"type": "string"},
                   "basis_amount": {"type": "number", "description": "Default 100 (or 1 for 'serving')"},
                   "basis_unit": {"type": "string", "description": "g, ml or serving (oz, cup etc. are converted)"},
                   "nutrients": nutrients_schema(), "servings": servings_schema(),
                   "barcode": {"type": "string"}, "aliases": {"type": "string"}, "notes": {"type": "string", "description": "Where the numbers came from"}}),
            &["name", "nutrients"], false),
        tool("update_food", "Update food or recipe",
            "Change a food or recipe. Only the given fields change; nutrients are merged key by key (null clears one). Past diary entries are recalculated unless update_past_entries is false.",
            json!({"food_id": {"type": "integer"}, "name": {"type": "string"}, "brand": {"type": ["string", "null"]},
                   "basis_amount": {"type": "number"}, "basis_unit": {"type": "string"},
                   "nutrients": nutrients_schema(), "servings": servings_schema(),
                   "barcode": {"type": ["string", "null"]}, "aliases": {"type": ["string", "null"]}, "notes": {"type": ["string", "null"]},
                   "ingredients": ingredients_schema(), "recipe_servings": {"type": "number"}, "cooked_weight_g": {"type": ["number", "null"]},
                   "update_past_entries": {"type": "boolean"}}),
            &["food_id"], false),
        tool("delete_food", "Delete food",
            "Remove a food from the library. Foods that were logged or are recipe ingredients are archived instead (history is kept).",
            json!({"food_id": {"type": "integer"}}), &["food_id"], false),
        tool("create_recipe", "Create recipe",
            "Create a shared recipe from library foods, e.g. 'Morning oats' = 50 g oats + 1 cup milk + 1 banana. Nutrients are summed from the ingredients.",
            json!({"name": {"type": "string"}, "ingredients": ingredients_schema(),
                   "servings": {"type": "number", "description": "How many servings the recipe makes (default 1)"},
                   "cooked_weight_g": {"type": "number", "description": "Total cooked weight, so portions can be logged in grams"},
                   "aliases": {"type": "string"}, "notes": {"type": "string"}}),
            &["name", "ingredients"], false),
        tool("log_food", "Log food",
            "Add a library food or recipe to a user's diary. Returns the entry and the day's totals vs budget.",
            json!({"user": user_prop(), "food_id": {"type": "integer"},
                   "quantity": {"type": "number", "description": "Default 1"},
                   "unit": {"type": "string", "description": "Serving label ('cup', 'bar'), g/oz/lb, ml/cup/tbsp/fl oz, or 'serving'. Default: the food's default serving."},
                   "serving_id": {"type": "integer"},
                   "meal": {"type": "string", "enum": ["breakfast", "lunch", "dinner", "snacks"]},
                   "date": date_prop("Diary day"), "time": time_prop(), "note": {"type": "string"}}),
            &["food_id"], false),
        tool("quick_add", "Quick add",
            "Log nutrients without a library food (restaurant meal, estimate). Prefer log_food when the food exists.",
            json!({"user": user_prop(), "name": {"type": "string"}, "nutrients": nutrients_schema(),
                   "meal": {"type": "string", "enum": ["breakfast", "lunch", "dinner", "snacks"]},
                   "date": date_prop("Diary day"), "time": time_prop(), "note": {"type": "string"}}),
            &["name", "nutrients"], false),
        tool("get_diary", "Get diary", "A user's entries for one day with totals, calorie budget, protein target and the weekly balance.",
            json!({"user": user_prop(), "date": date_prop("Day"), "include_micros": {"type": "boolean", "description": "Include vitamins/minerals vs daily values"}}),
            &[], true),
        tool("update_entry", "Update diary entry", "Change the quantity, unit, meal, date or note of a diary entry.",
            json!({"entry_id": {"type": "integer"}, "quantity": {"type": "number"}, "unit": {"type": "string"}, "serving_id": {"type": "integer"},
                   "meal": {"type": "string", "enum": ["breakfast", "lunch", "dinner", "snacks"]}, "date": {"type": "string"},
                   "time": {"type": ["string", "null"], "description": "When it was eaten, e.g. \"19:30\" or \"7:30 pm\"; null clears it"},
                   "note": {"type": ["string", "null"]},
                   "nutrients": nutrients_schema()}),
            &["entry_id"], false),
        tool("delete_entry", "Delete diary entry", "Remove an entry from a diary.", json!({"entry_id": {"type": "integer"}}), &["entry_id"], false),
        tool("copy_meal", "Copy meal", "Copy entries from one day (and optionally one meal or another user's diary) to another day.",
            json!({"user": user_prop(), "from_date": {"type": "string"}, "to_date": date_prop("Destination day"),
                   "meal": {"type": "string", "enum": ["breakfast", "lunch", "dinner", "snacks"]},
                   "to_meal": {"type": "string", "enum": ["breakfast", "lunch", "dinner", "snacks"]},
                   "from_user": {"type": ["string", "integer"], "description": "Copy from this user's diary (e.g. a shared dinner)"}}),
            &["from_date"], false),
        tool("log_exercise", "Log exercise", "Record calories burned. They add to the budget only if the user's goal says so.",
            json!({"user": user_prop(), "name": {"type": "string"}, "calories": {"type": "number"}, "duration_min": {"type": "number"}, "date": date_prop("Day")}),
            &["name", "calories"], false),
        tool("log_weight", "Log weight", "Record a weigh-in (one per day; logging again replaces it).",
            json!({"user": user_prop(), "weight": {"type": "number"}, "unit": {"type": "string", "enum": ["kg", "lb"], "description": "Default: the user's unit"},
                   "body_fat_pct": {"type": "number"}, "date": date_prop("Day"), "note": {"type": "string"}}),
            &["weight"], false),
        tool("get_weights", "Weight history", "Weigh-ins with the smoothed trend.",
            json!({"user": user_prop(), "from": {"type": "string"}, "to": {"type": "string"}, "limit": {"type": "integer"}}), &[], true),
        tool("get_goals", "Get goals", "Current calorie/macro goals, nutrient targets and maintenance-calorie estimates.",
            json!({"user": user_prop()}), &[], true),
        tool("set_goals", "Set goals",
            "Set daily goals from a date on (default today). Fields not given keep their current values; past days keep their old goals.",
            json!({"user": user_prop(), "calories": {"type": ["number", "null"]}, "protein": {"type": ["number", "null"], "description": "grams"},
                   "carbs": {"type": ["number", "null"]}, "fat": {"type": ["number", "null"]}, "fiber": {"type": ["number", "null"]},
                   "exercise_adds_to_budget": {"type": "boolean"}, "effective_from": {"type": "string"},
                   "micros": {"type": "object", "description": "Targets for other nutrients by key, e.g. {\"sodium\": 2000}; null removes"}}),
            &[], false),
        tool("get_summary", "Trends summary",
            "Averages and trends over a range: calories vs budget, protein hit rate and streak, macro split, weight trend and rate, estimated maintenance calories, micronutrient averages.",
            json!({"user": user_prop(), "from": {"type": "string"}, "to": {"type": "string"}, "days": {"type": "integer", "description": "Alternative to from: the last N days (default 30)"},
                   "include_days": {"type": "boolean", "description": "Include the per-day rows"}}),
            &[], true),
    ]
}

// ---------------------------------------------------------------------------------------------------------
// Argument helpers (lenient with numbers given as strings)

struct Args(Map<String, Value>);

impl Args {
    fn new(v: Value, allowed: &[&str]) -> AppResult<Args> {
        let Value::Object(m) = v else {
            return Err(bad("arguments must be an object"));
        };
        for k in m.keys() {
            if !allowed.contains(&k.as_str()) {
                return Err(bad(format!(
                    "unknown argument '{k}'. Allowed: {}",
                    allowed.join(", ")
                )));
            }
        }
        Ok(Args(m))
    }
    fn has(&self, k: &str) -> bool {
        self.0.contains_key(k)
    }
    fn is_null(&self, k: &str) -> bool {
        matches!(self.0.get(k), Some(Value::Null))
    }
    fn str(&self, k: &str) -> AppResult<Option<String>> {
        match self.0.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(Value::Number(n)) => Ok(Some(n.to_string())),
            Some(_) => Err(bad(format!("'{k}' must be a string"))),
        }
    }
    fn req_str(&self, k: &str) -> AppResult<String> {
        self.str(k)?
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| bad(format!("'{k}' is required")))
    }
    fn num(&self, k: &str) -> AppResult<Option<f64>> {
        match self.0.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(n)) => Ok(n.as_f64()),
            Some(Value::String(s)) => s
                .trim()
                .parse::<f64>()
                .map(Some)
                .map_err(|_| bad(format!("'{k}' must be a number"))),
            Some(_) => Err(bad(format!("'{k}' must be a number"))),
        }
    }
    fn int(&self, k: &str) -> AppResult<Option<i64>> {
        match self.num(k)? {
            None => Ok(None),
            Some(f) if f.fract() == 0.0 => Ok(Some(f as i64)),
            Some(_) => Err(bad(format!("'{k}' must be an integer"))),
        }
    }
    fn req_int(&self, k: &str) -> AppResult<i64> {
        self.int(k)?
            .ok_or_else(|| bad(format!("'{k}' is required")))
    }
    fn bool(&self, k: &str) -> AppResult<Option<bool>> {
        match self.0.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Bool(b)) => Ok(Some(*b)),
            Some(Value::String(s)) if s == "true" || s == "false" => Ok(Some(s == "true")),
            Some(_) => Err(bad(format!("'{k}' must be true or false"))),
        }
    }
    fn get(&self, k: &str) -> Option<&Value> {
        self.0.get(k)
    }
    fn typed<T: serde::de::DeserializeOwned>(&self, k: &str) -> AppResult<Option<T>> {
        match self.0.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => serde_json::from_value(v.clone())
                .map(Some)
                .map_err(|e| bad(format!("'{k}': {e}"))),
        }
    }
}

async fn resolve_user(state: &AppState, q: &McpQuery, a: &Args) -> AppResult<User> {
    let spec = a.str("user")?.or(q.user.clone());
    state
        .db(move |c| match spec {
            Some(s) => users::resolve(c, &s),
            None => {
                let all = users::list(c)?;
                match all.len() {
                    1 => Ok(all[0].clone()),
                    0 => Err(bad("there are no users yet; create one in the web UI")),
                    _ => Err(bad(format!(
                        "which user? Pass `user`: {}",
                        all.iter()
                            .map(|u| u.name.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))),
                }
            }
        })
        .await
}

fn r1(v: Option<f64>) -> Value {
    v.map(|x| json!(round_to(x, 1))).unwrap_or(Value::Null)
}

fn brief(n: &Nutrients) -> Value {
    json!({"calories": r1(n.get("calories")), "protein": r1(n.get("protein")), "carbs": r1(n.get("carbs")),
           "fat": r1(n.get("fat")), "fiber": r1(n.get("fiber"))})
}

fn food_brief(f: &foods::Food) -> Value {
    json!({
        "id": f.id, "name": f.name, "brand": f.brand, "kind": f.kind, "source": f.source,
        "nutrients_per": f.basis_label(), "per_basis": brief(&f.nutrients), "nutrient_count": f.nutrients.known_count(),
        "servings": f.servings.iter().map(|s| json!({"id": s.id, "label": s.label, "amount": format!("{} {}", foods::fmt_num(s.amount), f.basis_unit), "default": s.is_default})).collect::<Vec<_>>(),
        "per_default_serving": f.per_serving.as_ref().map(|p| json!({"serving": p.label, "calories": r1(p.calories), "protein": r1(p.protein), "carbs": r1(p.carbs), "fat": r1(p.fat)})),
        "units": f.units, "aliases": f.aliases, "barcode": f.barcode, "archived": f.archived,
    })
}

fn day_brief(d: &diary::Diary) -> Value {
    let protein_t = d
        .targets
        .iter()
        .find(|t| t.key == "protein")
        .filter(|t| t.from == "goal")
        .map(|t| t.target);
    json!({
        "date": d.date,
        "eaten": brief(&d.totals),
        "calorie_budget": d.budget.budget,
        "calories_remaining": d.budget.remaining,
        "exercise_calories": d.budget.exercise,
        "protein_target": protein_t,
        "protein_remaining": protein_t.map(|t| round_to(t - d.totals.val("protein"), 1)),
    })
}

fn entry_brief(e: &diary::Entry) -> Value {
    json!({"id": e.id, "date": e.date, "meal": e.meal, "time": e.time, "food_id": e.food_id, "name": e.name, "brand": e.brand,
           "quantity": e.display_quantity, "nutrients": brief(&e.nutrients), "note": e.note})
}

// ---------------------------------------------------------------------------------------------------------
// Tool implementations

pub async fn call_tool(
    state: &AppState,
    q: &McpQuery,
    name: &str,
    args: Value,
) -> AppResult<Value> {
    match name {
        "list_users" => {
            Args::new(args, &[])?;
            state
                .db(|c| {
                    let mut out = vec![];
                    for u in users::list(c)? {
                        let today = user_now(&u).0;
                        let g = goals::for_date(c, u.id, &fmt_date(today))?;
                        out.push(json!({"id": u.id, "name": u.name, "weight_unit": u.weight_unit, "today": fmt_date(today),
                            "goal": g.map(|g| json!({"calories": g.calories, "protein": g.protein, "carbs": g.carbs, "fat": g.fat, "fiber": g.fiber}))}));
                    }
                    Ok(json!({"users": out}))
                })
                .await
        }
        "get_nutrient_catalog" => {
            Args::new(args, &[])?;
            Ok(
                json!({"nutrients": CATALOG.iter().map(|n| json!({"key": n.key, "label": n.label, "unit": n.unit, "daily_value": n.dv, "is_limit": n.limit, "aliases": n.aliases})).collect::<Vec<_>>()}),
            )
        }
        "search_foods" => {
            let a = Args::new(args, &["query", "kind", "limit"])?;
            let query = a.str("query")?.unwrap_or_default();
            let kind = a.str("kind")?;
            let limit = a.int("limit")?.unwrap_or(10).clamp(1, 50) as usize;
            let uid = resolve_user(state, q, &Args(Map::new()))
                .await
                .ok()
                .map(|u| u.id);
            let hits = state
                .db(move |c| {
                    foods::search(
                        c,
                        SearchOpts {
                            q: &query,
                            kind: kind.as_deref(),
                            limit,
                            user_id: uid,
                            include_archived: false,
                        },
                    )
                })
                .await?;
            Ok(
                json!({"count": hits.len(), "foods": hits.iter().map(|h| { let mut v = food_brief(&h.food); v["times_logged"] = json!(h.uses); v }).collect::<Vec<_>>(),
                "hint": if hits.is_empty() { Some("Nothing in the library. Try search_online_foods, then create_food.") } else { None }}),
            )
        }
        "get_food" => {
            let a = Args::new(args, &["food_id"])?;
            let id = a.req_int("food_id")?;
            let f = state.db(move |c| foods::get(c, id)).await?;
            let mut v = serde_json::to_value(&f)?;
            v["nutrients_per"] = json!(f.basis_label());
            Ok(v)
        }
        "search_online_foods" => {
            let a = Args::new(args, &["query", "source", "usda_kind", "limit"])?;
            let src = Source::parse(a.str("source")?.as_deref())?;
            let kind = a.str("usda_kind")?.unwrap_or_else(|| "all".into());
            let limit = a.int("limit")?.unwrap_or(8).clamp(1, 25) as usize;
            let r = external::search(state, &a.req_str("query")?, src, limit, &kind).await?;
            Ok(json!({
                "results": r.results.iter().map(|c| json!({
                    "source": c.source, "id": c.source_id, "name": c.name, "brand": c.brand, "barcode": c.barcode,
                    "type": c.data_type, "category": c.category, "nutrients_per": format!("{} {}", foods::fmt_num(c.basis_amount), c.basis_unit),
                    "per_basis": brief(&c.nutrients), "nutrient_count": c.nutrient_count,
                    "servings": c.servings.iter().map(|s| format!("{} = {} {}", s.label, foods::fmt_num(s.amount), c.basis_unit)).collect::<Vec<_>>(),
                    "already_in_library_as_food_id": c.in_library,
                })).collect::<Vec<_>>(),
                "errors": r.errors,
            }))
        }
        "lookup_barcode" => {
            let a = Args::new(args, &["barcode"])?;
            let r = external::barcode(state, &a.req_str("barcode")?).await?;
            Ok(
                json!({"barcode": r.barcode, "library_food": r.food.as_ref().map(food_brief),
                "online": r.candidate.as_ref().map(|c| json!({"source": c.source, "id": c.source_id, "name": c.name, "brand": c.brand,
                    "nutrients_per": format!("{} {}", foods::fmt_num(c.basis_amount), c.basis_unit), "per_basis": brief(&c.nutrients),
                    "already_in_library_as_food_id": c.in_library})),
                "errors": r.errors,
                "hint": if r.food.is_none() && r.candidate.is_none() { Some("Not found anywhere; create_food with the label values and this barcode.") } else { None }}),
            )
        }
        "import_online_food" => {
            let a = Args::new(args, &["source", "id", "aliases", "name"])?;
            let uid = resolve_user(state, q, &Args(Map::new()))
                .await
                .ok()
                .map(|u| u.id);
            let r = external::import(
                state,
                &a.req_str("source")?,
                &a.req_str("id")?,
                uid,
                a.str("aliases")?,
                a.str("name")?,
            )
            .await?;
            Ok(json!({"created": r.created, "food": food_brief(&r.food), "warnings": r.warnings}))
        }
        "create_food" => {
            let a = Args::new(
                args,
                &[
                    "name",
                    "brand",
                    "basis_amount",
                    "basis_unit",
                    "nutrients",
                    "servings",
                    "barcode",
                    "aliases",
                    "notes",
                ],
            )?;
            let nutrients =
                Nutrients::from_json(a.get("nutrients").unwrap_or(&Value::Null)).map_err(bad)?;
            let inp = FoodInput {
                name: a.req_str("name")?,
                brand: a.str("brand")?,
                kind: Some("food".into()),
                basis_amount: a.num("basis_amount")?,
                basis_unit: a.str("basis_unit")?,
                barcode: a.str("barcode")?,
                aliases: a.str("aliases")?,
                notes: a.str("notes")?,
                nutrients,
                servings: a.typed::<Vec<ServingInput>>("servings")?,
                source: Some("ai".into()),
                source_id: None,
                ingredients: None,
                recipe_yield: None,
                cooked_weight_g: None,
            };
            let uid = resolve_user(state, q, &Args(Map::new()))
                .await
                .ok()
                .map(|u| u.id);
            let saved = state.db(move |c| foods::create(c, inp, uid)).await?;
            Ok(json!({"food": food_brief(&saved.food), "warnings": saved.warnings}))
        }
        "update_food" => {
            let a = Args::new(
                args,
                &[
                    "food_id",
                    "name",
                    "brand",
                    "basis_amount",
                    "basis_unit",
                    "nutrients",
                    "servings",
                    "barcode",
                    "aliases",
                    "notes",
                    "ingredients",
                    "recipe_servings",
                    "cooked_weight_g",
                    "update_past_entries",
                ],
            )?;
            let id = a.req_int("food_id")?;
            let propagate = a.bool("update_past_entries")?.unwrap_or(true);
            let patch_n = match a.get("nutrients") {
                Some(v) if !v.is_null() => Some(v.clone()),
                _ => None,
            };
            let name = a.str("name")?;
            let basis_amount = a.num("basis_amount")?;
            let basis_unit = a.str("basis_unit")?;
            let servings = a.typed::<Vec<ServingInput>>("servings")?;
            let ingredients = a.typed::<Vec<IngredientInput>>("ingredients")?;
            let recipe_servings = a.num("recipe_servings")?;
            let cooked = if a.is_null("cooked_weight_g") {
                Some(None)
            } else {
                a.num("cooked_weight_g")?.map(Some)
            };
            let (brand_a, barcode_a, aliases_a, notes_a) = (
                a.has("brand"),
                a.has("barcode"),
                a.has("aliases"),
                a.has("notes"),
            );
            let (brand_v, barcode_v, aliases_v, notes_v) = (
                a.str("brand")?,
                a.str("barcode")?,
                a.str("aliases")?,
                a.str("notes")?,
            );
            let saved = state
                .db(move |c| {
                    let f = foods::get(c, id)?;
                    let mut inp = foods::to_input(&f);
                    if let Some(n) = name {
                        inp.name = n;
                    }
                    if brand_a {
                        inp.brand = brand_v;
                    }
                    if barcode_a {
                        inp.barcode = barcode_v;
                    }
                    if aliases_a {
                        inp.aliases = aliases_v;
                    }
                    if notes_a {
                        inp.notes = notes_v;
                    }
                    if let Some(b) = basis_amount {
                        inp.basis_amount = Some(b);
                    }
                    if let Some(u) = basis_unit {
                        inp.basis_unit = Some(u);
                    }
                    if let Some(pn) = patch_n {
                        if f.kind == "recipe" {
                            return Err(bad("recipe nutrients come from the ingredients; change the ingredients instead"));
                        }
                        let obj = pn.as_object().ok_or_else(|| bad("nutrients must be an object"))?;
                        let mut merged = serde_json::to_value(&inp.nutrients)?;
                        for (k, v) in obj {
                            let i = crate::nutrients::resolve_key(k).ok_or_else(|| bad(format!("unknown nutrient '{k}'")))?;
                            merged[CATALOG[i].key] = v.clone();
                        }
                        inp.nutrients = Nutrients::from_json(&merged).map_err(bad)?;
                    }
                    if let Some(s) = servings {
                        if f.kind == "recipe" {
                            return Err(bad("recipe servings are derived from recipe_servings / cooked_weight_g"));
                        }
                        inp.servings = Some(s);
                    }
                    if f.kind == "recipe" {
                        if let Some(i) = ingredients {
                            inp.ingredients = Some(i);
                        }
                        if let Some(y) = recipe_servings {
                            inp.recipe_yield = Some(y);
                        }
                        if let Some(cw) = cooked {
                            inp.cooked_weight_g = cw;
                        }
                    } else if ingredients.is_some() || recipe_servings.is_some() {
                        return Err(bad("ingredients/recipe_servings only apply to recipes"));
                    }
                    foods::update(c, id, inp, propagate)
                })
                .await?;
            Ok(
                json!({"food": food_brief(&saved.food), "warnings": saved.warnings, "past_entries_updated": saved.entries_updated}),
            )
        }
        "delete_food" => {
            let a = Args::new(args, &["food_id"])?;
            let id = a.req_int("food_id")?;
            let archived = state.db(move |c| foods::delete(c, id)).await?;
            Ok(
                json!({"food_id": id, "result": if archived { "archived (it is used in diaries or recipes)" } else { "deleted" }}),
            )
        }
        "create_recipe" => {
            let a = Args::new(
                args,
                &[
                    "name",
                    "ingredients",
                    "servings",
                    "cooked_weight_g",
                    "aliases",
                    "notes",
                ],
            )?;
            let ingredients = a
                .typed::<Vec<IngredientInput>>("ingredients")?
                .ok_or_else(|| bad("'ingredients' is required"))?;
            let inp = FoodInput {
                name: a.req_str("name")?,
                kind: Some("recipe".into()),
                aliases: a.str("aliases")?,
                notes: a.str("notes")?,
                ingredients: Some(ingredients),
                recipe_yield: a.num("servings")?,
                cooked_weight_g: a.num("cooked_weight_g")?,
                ..Default::default()
            };
            let uid = resolve_user(state, q, &Args(Map::new()))
                .await
                .ok()
                .map(|u| u.id);
            let saved = state.db(move |c| foods::create(c, inp, uid)).await?;
            let mut v = food_brief(&saved.food);
            v["ingredients"] = json!(saved.food.ingredients.as_ref().map(|items| items.iter().map(|i| json!({"food_id": i.food_id, "name": i.name, "quantity": format!("{} {}", foods::fmt_num(i.quantity), i.unit), "calories": r1(i.nutrients.get("calories")), "protein": r1(i.nutrients.get("protein"))})).collect::<Vec<_>>()));
            Ok(json!({"recipe": v, "warnings": saved.warnings}))
        }
        "log_food" | "quick_add" => {
            let a = if name == "log_food" {
                Args::new(
                    args,
                    &[
                        "user",
                        "food_id",
                        "quantity",
                        "unit",
                        "serving_id",
                        "meal",
                        "date",
                        "time",
                        "note",
                    ],
                )?
            } else {
                Args::new(
                    args,
                    &["user", "name", "nutrients", "meal", "date", "time", "note"],
                )?
            };
            let time = if a.has("time") {
                Some(a.str("time")?)
            } else {
                None
            };
            let u = resolve_user(state, q, &a).await?;
            let inp = if name == "log_food" {
                EntryInput {
                    date: a.str("date")?,
                    meal: a.str("meal")?,
                    food_id: Some(a.req_int("food_id")?),
                    quantity: a.num("quantity")?,
                    unit: a.str("unit")?,
                    serving_id: a.int("serving_id")?,
                    note: a.str("note")?,
                    source: Some("mcp".into()),
                    time: time.clone(),
                    ..Default::default()
                }
            } else {
                let n = Nutrients::from_json(a.get("nutrients").unwrap_or(&Value::Null))
                    .map_err(bad)?;
                EntryInput {
                    date: a.str("date")?,
                    meal: a.str("meal")?,
                    name: Some(a.req_str("name")?),
                    nutrients: Some(n),
                    note: a.str("note")?,
                    source: Some("mcp".into()),
                    time,
                    ..Default::default()
                }
            };
            state
                .db(move |c| {
                    let (today, now) = user_now(&u);
                    let e = diary::add_entry(c, u.id, inp, today, now)?;
                    let d = diary::diary(c, u.id, crate::util::parse_date(&e.date)?)?;
                    Ok(json!({"user": u.name, "logged": entry_brief(&e), "day": day_brief(&d)}))
                })
                .await
        }
        "get_diary" => {
            let a = Args::new(args, &["user", "date", "include_micros"])?;
            let u = resolve_user(state, q, &a).await?;
            let date = a.str("date")?;
            let micros = a.bool("include_micros")?.unwrap_or(false);
            state
                .db(move |c| {
                    let d = diary::diary(c, u.id, resolve_date(date.as_deref(), user_now(&u).0)?)?;
                    let mut v = day_brief(&d);
                    v["user"] = json!(u.name);
                    v["entries"] = json!(d.entries.iter().map(entry_brief).collect::<Vec<_>>());
                    v["meals"] = json!(d.meals.iter().map(|(k, n)| (k.clone(), brief(n))).collect::<Map<String, Value>>());
                    v["exercises"] = json!(d.exercises);
                    v["meal_times"] = json!(d.timing.occasions);
                    v["usual_meal_times"] = json!(d.timing.usual);
                    v["eating_window"] = json!(d.timing.first.as_ref().map(|f| json!({"first": f, "last": d.timing.last})));
                    v["week"] = json!(d.week);
                    v["weight_kg"] = json!(d.weight.as_ref().map(|w| w.weight_kg));
                    if micros {
                        v["micros"] = json!(d
                            .targets
                            .iter()
                            .filter(|t| !matches!(t.key, "protein" | "carbs" | "fat" | "fiber"))
                            .map(|t| {
                                let have = d.totals.get(t.key);
                                json!({"key": t.key, "amount": have.map(|x| round_to(x, 2)), "target": t.target, "limit": t.limit,
                                       "pct": have.map(|x| round_to(x / t.target * 100.0, 0)), "coverage": d.coverage.get(t.key)})
                            })
                            .collect::<Vec<_>>());
                    }
                    Ok(v)
                })
                .await
        }
        "update_entry" => {
            let a = Args::new(
                args,
                &[
                    "entry_id",
                    "quantity",
                    "unit",
                    "serving_id",
                    "meal",
                    "date",
                    "time",
                    "note",
                    "nutrients",
                ],
            )?;
            let id = a.req_int("entry_id")?;
            let time = if a.has("time") {
                Some(a.str("time")?)
            } else {
                None
            };
            let p = EntryPatch {
                date: a.str("date")?,
                meal: a.str("meal")?,
                quantity: a.num("quantity")?,
                unit: a.str("unit")?,
                serving_id: a.int("serving_id")?.map(Some),
                note: if a.has("note") {
                    Some(a.str("note")?)
                } else {
                    None
                },
                name: None,
                time,
                nutrients: match a.get("nutrients") {
                    Some(v) if !v.is_null() => Some(Nutrients::from_json(v).map_err(bad)?),
                    _ => None,
                },
            };
            state
                .db(move |c| {
                    let e = diary::update_entry(c, id, p)?;
                    let d = diary::diary(c, e.user_id, crate::util::parse_date(&e.date)?)?;
                    Ok(json!({"entry": entry_brief(&e), "day": day_brief(&d)}))
                })
                .await
        }
        "delete_entry" => {
            let a = Args::new(args, &["entry_id"])?;
            let id = a.req_int("entry_id")?;
            state
                .db(move |c| {
                    let e = diary::delete_entry(c, id)?;
                    let d = diary::diary(c, e.user_id, crate::util::parse_date(&e.date)?)?;
                    Ok(json!({"deleted": entry_brief(&e), "day": day_brief(&d)}))
                })
                .await
        }
        "copy_meal" => {
            let a = Args::new(
                args,
                &[
                    "user",
                    "from_date",
                    "to_date",
                    "meal",
                    "to_meal",
                    "from_user",
                ],
            )?;
            let u = resolve_user(state, q, &a).await?;
            let from_user = match a.str("from_user")? {
                Some(s) => Some(state.db(move |c| users::resolve(c, &s)).await?.id),
                None => None,
            };
            let (fd, td, meal, to_meal) = (
                a.req_str("from_date")?,
                a.str("to_date")?,
                a.str("meal")?,
                a.str("to_meal")?,
            );
            state
                .db(move |c| {
                    let today = user_now(&u).0;
                    let inp = CopyInput {
                        from_date: fmt_date(resolve_date(Some(&fd), today)?),
                        to_date: fmt_date(resolve_date(td.as_deref(), today)?),
                        meal,
                        to_meal,
                        from_user_id: from_user,
                    };
                    let to = inp.to_date.clone();
                    let copied = diary::copy_entries(c, u.id, inp)?;
                    let d = diary::diary(c, u.id, crate::util::parse_date(&to)?)?;
                    Ok(json!({"copied": copied.iter().map(entry_brief).collect::<Vec<_>>(), "day": day_brief(&d)}))
                })
                .await
        }
        "log_exercise" => {
            let a = Args::new(args, &["user", "name", "calories", "duration_min", "date"])?;
            let u = resolve_user(state, q, &a).await?;
            let inp = ExerciseInput {
                date: a.str("date")?,
                name: a.req_str("name")?,
                calories: a
                    .num("calories")?
                    .ok_or_else(|| bad("'calories' is required"))?,
                duration_min: a.num("duration_min")?,
            };
            state
                .db(move |c| {
                    let e = diary::add_exercise(c, u.id, inp, user_now(&u).0)?;
                    let d = diary::diary(c, u.id, crate::util::parse_date(&e.date)?)?;
                    Ok(json!({"exercise": e, "day": day_brief(&d)}))
                })
                .await
        }
        "log_weight" => {
            let a = Args::new(
                args,
                &["user", "weight", "unit", "body_fat_pct", "date", "note"],
            )?;
            let u = resolve_user(state, q, &a).await?;
            // Accept 81.2, "81.2", "81.2 kg" or "180lb" (a unit in the value is used when `unit` is absent).
            let (weight, unit_in_value) = match a.get("weight") {
                Some(Value::String(s)) => {
                    let t = s.trim().to_lowercase();
                    let split = t.find(|c: char| c.is_alphabetic()).unwrap_or(t.len());
                    let (n, u) = t.split_at(split);
                    let n: f64 = n
                        .trim()
                        .parse()
                        .map_err(|_| bad("'weight' must be a number like 81.2 or \"180 lb\""))?;
                    (n, Some(u.trim().to_string()).filter(|u| !u.is_empty()))
                }
                _ => (
                    a.num("weight")?
                        .ok_or_else(|| bad("'weight' is required"))?,
                    None,
                ),
            };
            let inp = WeightInput {
                date: a.str("date")?,
                weight,
                unit: a.str("unit")?.or(unit_in_value),
                body_fat_pct: a.num("body_fat_pct")?,
                note: a.str("note")?,
            };
            state
                .db(move |c| {
                    let w = weights::upsert(c, u.id, &u.weight_unit, inp, user_now(&u).0)?;
                    let all = weights::list(c, u.id, None, Some(&w.date))?;
                    let trend = weights::trend(&all).last().map(|t| t.2);
                    Ok(json!({"user": u.name, "weight": w, "trend_kg": trend,
                        "display": if u.weight_unit == "lb" { format!("{:.1} lb", w.weight_kg / crate::units::KG_PER_LB) } else { format!("{:.1} kg", w.weight_kg) }}))
                })
                .await
        }
        "get_weights" => {
            let a = Args::new(args, &["user", "from", "to", "limit"])?;
            let u = resolve_user(state, q, &a).await?;
            let (from, to, limit) = (
                a.str("from")?,
                a.str("to")?,
                a.int("limit")?.unwrap_or(60).clamp(1, 1000) as usize,
            );
            state
                .db(move |c| {
                    let all = weights::list(c, u.id, None, to.as_deref())?;
                    let tr = weights::trend(&all);
                    let f = from.unwrap_or_default();
                    let rows: Vec<Value> = all
                        .iter()
                        .zip(tr.iter())
                        .filter(|(w, _)| w.date >= f)
                        .map(|(w, t)| json!({"id": w.id, "date": w.date, "weight_kg": w.weight_kg, "trend_kg": t.2, "body_fat_pct": w.body_fat_pct}))
                        .collect();
                    let skip = rows.len().saturating_sub(limit);
                    Ok(json!({"user": u.name, "unit_preference": u.weight_unit, "goal_weight_kg": u.goal_weight_kg, "weights": rows[skip..].to_vec()}))
                })
                .await
        }
        "get_goals" => {
            let a = Args::new(args, &["user"])?;
            let u = resolve_user(state, q, &a).await?;
            state
                .db(move |c| {
                    let today = user_now(&u).0;
                    let all = goals::list(c, u.id)?;
                    let cur = goals::pick(&all, &fmt_date(today)).cloned();
                    let e = summary::energy(c, &u, today)?;
                    Ok(json!({"user": u.name, "current": cur, "targets": goals::targets(cur.as_ref()), "history": all, "energy": e}))
                })
                .await
        }
        "set_goals" => {
            let a = Args::new(
                args,
                &[
                    "user",
                    "calories",
                    "protein",
                    "carbs",
                    "fat",
                    "fiber",
                    "exercise_adds_to_budget",
                    "effective_from",
                    "micros",
                ],
            )?;
            let u = resolve_user(state, q, &a).await?;
            let opt = |k: &str| -> AppResult<Option<Option<f64>>> {
                if a.is_null(k) {
                    Ok(Some(None))
                } else {
                    Ok(a.num(k)?.map(Some))
                }
            };
            let inp = GoalInput {
                effective_from: a.str("effective_from")?,
                calories: opt("calories")?,
                protein: opt("protein")?,
                carbs: opt("carbs")?,
                fat: opt("fat")?,
                fiber: opt("fiber")?,
                exercise_adds_to_budget: a.bool("exercise_adds_to_budget")?,
                micros: a.typed("micros")?,
            };
            state
                .db(move |c| {
                    Ok(json!({"user": u.name, "goal": goals::set(c, u.id, inp, user_now(&u).0)?}))
                })
                .await
        }
        "get_summary" => {
            let a = Args::new(args, &["user", "from", "to", "days", "include_days"])?;
            let u = resolve_user(state, q, &a).await?;
            let (from, to, days, include) = (
                a.str("from")?,
                a.str("to")?,
                a.int("days")?,
                a.bool("include_days")?.unwrap_or(false),
            );
            state
                .db(move |c| {
                    let today = user_now(&u).0;
                    let to = resolve_date(to.as_deref(), today)?;
                    let from = match from {
                        Some(f) => resolve_date(Some(&f), today)?,
                        None => to - chrono::Duration::days(days.unwrap_or(30).clamp(1, 1000) - 1),
                    };
                    let s = summary::summary(c, &u, from, to, today)?;
                    let mut v = json!({"user": u.name, "from": s.from, "to": s.to, "stats": s.stats,
                        "micros": s.micros.iter().filter(|m| m.avg.is_some()).map(|m| json!({"key": m.key, "avg": m.avg.map(|x| round_to(x, 2)), "unit": m.unit, "pct_of_target": m.pct, "is_limit": m.limit, "days_with_data": m.days_with_data})).collect::<Vec<_>>()});
                    if include {
                        v["days"] = json!(s.days.iter().map(|d| json!({"date": d.date, "logged": d.logged, "calories": r1(d.nutrients.get("calories")), "protein": r1(d.nutrients.get("protein")), "budget": d.budget, "protein_target": d.protein_target, "weight_kg": d.weight_kg, "trend_kg": d.trend_kg})).collect::<Vec<_>>());
                    }
                    Ok(v)
                })
                .await
        }
        other => Err(AppError::NotFound(format!("unknown tool {other}"))),
    }
}
