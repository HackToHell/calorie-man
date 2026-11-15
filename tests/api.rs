//! End-to-end tests of the REST API and MCP endpoint against a temporary database and a mock
//! USDA / Open Food Facts server that serves saved real responses.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use calorie_man::config::Config;
use calorie_man::AppState;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

static FLAKY_USDA: AtomicUsize = AtomicUsize::new(0);

async fn mock_external() -> String {
    let app = Router::new()
        .route(
            "/fdc/foods/search",
            get(|| async {
                (
                    [("content-type", "application/json")],
                    fixture("usda_search_kirkland_milk.json"),
                )
            }),
        )
        .route(
            "/fdc/food/{id}",
            get(
                |axum::extract::Path(id): axum::extract::Path<String>| async move {
                    match id.as_str() {
                        // USDA's nginx intermittently answers identical requests with an HTML 400.
                        "2709223" if FLAKY_USDA.fetch_add(1, Ordering::SeqCst) == 0 => (
                            StatusCode::BAD_REQUEST,
                            "<html><head><title>400 Bad Request</title></head></html>".to_string(),
                        ),
                        "2709223" => (StatusCode::OK, fixture("usda_food_2709223.json")),
                        "2460407" => (StatusCode::OK, fixture("usda_food_2460407.json")),
                        "173944" => (StatusCode::OK, fixture("usda_food_173944.json")),
                        _ => (StatusCode::NOT_FOUND, "{}".to_string()),
                    }
                },
            ),
        )
        .route(
            "/off/api/v2/product/{file}",
            get(
                |axum::extract::Path(file): axum::extract::Path<String>| async move {
                    match file.as_str() {
                        "3017624010701.json" => fixture("off_product_nutella.json"),
                        "0096619204205.json" | "096619204205.json" => {
                            fixture("off_product_kirkland.json")
                        }
                        _ => r#"{"status":0,"status_verbose":"product not found"}"#.to_string(),
                    }
                },
            ),
        )
        .route(
            "/offsearch/search",
            get(|| async { fixture("off_search_kirkland_milk.json") }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

struct T {
    app: Router,
    _dir: tempfile::TempDir,
}

impl T {
    async fn new() -> T {
        let dir = tempfile::tempdir().unwrap();
        let base = mock_external().await;
        let mut cfg = Config::new(dir.path().join("t.db"));
        cfg.usda_base = format!("{base}/fdc");
        cfg.off_base = format!("{base}/off");
        cfg.off_search_base = format!("{base}/offsearch");
        cfg.offline = false;
        let state = AppState::new(cfg).unwrap();
        T {
            app: calorie_man::app(state),
            _dir: dir,
        }
    }

    async fn req(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut b = Request::builder().method(method).uri(uri);
        if body.is_some() {
            b = b.header("content-type", "application/json");
        }
        let req = b
            .body(
                body.map(|v| Body::from(v.to_string()))
                    .unwrap_or(Body::empty()),
            )
            .unwrap();
        let resp = self.app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v = serde_json::from_slice(&bytes)
            .unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, v)
    }
    async fn get(&self, uri: &str) -> Value {
        let (s, v) = self.req("GET", uri, None).await;
        assert!(s.is_success(), "GET {uri} -> {s}: {v}");
        v
    }
    async fn post(&self, uri: &str, body: Value) -> Value {
        let (s, v) = self.req("POST", uri, Some(body)).await;
        assert!(s.is_success(), "POST {uri} -> {s}: {v}");
        v
    }
    async fn put(&self, uri: &str, body: Value) -> Value {
        let (s, v) = self.req("PUT", uri, Some(body)).await;
        assert!(s.is_success(), "PUT {uri} -> {s}: {v}");
        v
    }
    async fn mcp(&self, method: &str, params: Value) -> Value {
        let (s, v) = self
            .req(
                "POST",
                "/mcp",
                Some(json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})),
            )
            .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        v
    }
    async fn tool(&self, name: &str, args: Value) -> (bool, Value) {
        let v = self
            .mcp("tools/call", json!({"name": name, "arguments": args}))
            .await;
        let res = &v["result"];
        let text = res["content"][0]["text"].as_str().unwrap_or("").to_string();
        let is_err = res["isError"].as_bool().unwrap_or(false);
        (
            is_err,
            serde_json::from_str(&text).unwrap_or(Value::String(text)),
        )
    }
    async fn ok_tool(&self, name: &str, args: Value) -> Value {
        let (err, v) = self.tool(name, args.clone()).await;
        assert!(!err, "{name}({args}) failed: {v}");
        v
    }
}

fn approx(a: &Value, b: f64) -> bool {
    (a.as_f64().unwrap_or(f64::NAN) - b).abs() < 0.05
}

async fn two_users(t: &T) -> (i64, i64) {
    let a = t.post("/api/users", json!({"name": "Jordan"})).await["id"]
        .as_i64()
        .unwrap();
    let b = t
        .post("/api/users", json!({"name": "Riley", "weight_unit": "lb"}))
        .await["id"]
        .as_i64()
        .unwrap();
    (a, b)
}

async fn oats_milk_banana(t: &T) -> (i64, i64, i64) {
    let oats = t
        .post("/api/foods", json!({"name": "Rolled oats", "nutrients": {"calories": 379, "protein": 13.2, "carbs": 67.7, "fat": 6.5, "fiber": 10.1, "iron": 4.3},
            "servings": [{"label": "1/2 cup", "amount": 40}]}))
        .await["food"]["id"]
        .as_i64()
        .unwrap();
    let milk = t
        .post("/api/foods", json!({"name": "Whole milk", "brand": "Kirkland Signature", "basis_unit": "ml",
            "nutrients": {"calories": 62, "protein": 3.33, "carbs": 5, "fat": 3.33, "calcium": 117, "vitamin_d": 1.25},
            "servings": [{"label": "cup", "amount": 240}], "aliases": "costco milk"}))
        .await["food"]["id"]
        .as_i64()
        .unwrap();
    let banana = t
        .post("/api/foods", json!({"name": "Banana", "nutrients": {"calories": 89, "protein": 1.09, "carbs": 22.8, "fat": 0.33, "potassium": 358},
            "servings": [{"label": "medium", "amount": 118}]}))
        .await["food"]["id"]
        .as_i64()
        .unwrap();
    (oats, milk, banana)
}

#[tokio::test]
async fn users_crud_and_validation() {
    let t = T::new().await;
    let (a, _) = two_users(&t).await;
    let (s, v) = t
        .req("POST", "/api/users", Some(json!({"name": "jordan"})))
        .await;
    assert_eq!(
        s,
        StatusCode::BAD_REQUEST,
        "duplicate names are case-insensitive: {v}"
    );
    let (s, _) = t.req("POST", "/api/users", Some(json!({"name": ""}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, v) = t
        .req(
            "PUT",
            &format!("/api/users/{a}"),
            Some(json!({"height_cm": 900})),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    let (s, _) = t
        .req(
            "PUT",
            &format!("/api/users/{a}"),
            Some(json!({"timezone": "Mars/Base"})),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let u = t.put(&format!("/api/users/{a}"), json!({"sex": "male", "birth_date": "1990-01-31", "height_cm": 178, "timezone": "America/Los_Angeles"})).await;
    assert_eq!(u["height_cm"], 178.0);
    let u = t
        .put(&format!("/api/users/{a}"), json!({"height_cm": null}))
        .await;
    assert!(u["height_cm"].is_null(), "null clears");
    assert_eq!(u["sex"], "male", "missing fields are kept");
    assert_eq!(t.get("/api/users").await.as_array().unwrap().len(), 2);
    let (s, _) = t.req("DELETE", &format!("/api/users/{a}"), None).await;
    assert!(s.is_success());
    assert_eq!(t.get("/api/users").await.as_array().unwrap().len(), 1);
    let (s, v) = t.req("GET", "/api/nope", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert!(v["error"].is_string());
}

#[tokio::test]
async fn foods_servings_search_and_sanity() {
    let t = T::new().await;
    two_users(&t).await;
    let (_, milk, _) = oats_milk_banana(&t).await;
    let hits = t.get("/api/foods?q=costco%20milk").await;
    assert_eq!(hits[0]["id"], milk, "alias + synonym search");
    let hits = t.get("/api/foods?q=kirkland").await;
    assert_eq!(hits[0]["id"], milk);
    assert_eq!(t.get("/api/foods?q=zzz").await.as_array().unwrap().len(), 0);
    let f = t.get(&format!("/api/foods/{milk}")).await;
    assert!(approx(&f["per_serving"]["calories"], 148.8));
    assert!(f["units"].as_array().unwrap().iter().any(|u| u == "fl oz"));

    // impossible macros are rejected, odd calories only warn
    let (s, v) = t
        .req(
            "POST",
            "/api/foods",
            Some(json!({"name": "Bad", "nutrients": {"protein": 80, "carbs": 80}})),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    let v = t.post("/api/foods", json!({"name": "Odd", "nutrients": {"calories": 500, "protein": 1, "carbs": 1, "fat": 1}})).await;
    assert!(v["warnings"][0].as_str().unwrap().contains("do not match"));
    let (s, v) = t
        .req(
            "POST",
            "/api/foods",
            Some(json!({"name": "X", "nutrients": {"protien": 3}})),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(v["error"].as_str().unwrap().contains("protien"), "{v}");
    // per-serving basis in ounces converts to grams
    let v = t.post("/api/foods", json!({"name": "Jerky", "basis_amount": 1, "basis_unit": "oz", "nutrients": {"calories": 80, "protein": 12}})).await;
    assert_eq!(v["food"]["basis_unit"], "g");
    assert!(approx(&v["food"]["basis_amount"], 28.35));
    // barcode uniqueness
    t.post(
        "/api/foods",
        json!({"name": "Bar", "barcode": "012345678905", "nutrients": {"calories": 200}}),
    )
    .await;
    let (s, _) = t
        .req(
            "POST",
            "/api/foods",
            Some(
                json!({"name": "Bar 2", "barcode": "12345678905", "nutrients": {"calories": 200}}),
            ),
        )
        .await;
    assert_eq!(
        s,
        StatusCode::CONFLICT,
        "leading zeros do not make a different barcode"
    );
}

#[tokio::test]
async fn diary_logging_budget_and_week() {
    let t = T::new().await;
    let (a, b) = two_users(&t).await;
    let (oats, milk, _) = oats_milk_banana(&t).await;
    t.post(&format!("/api/users/{a}/goals"), json!({"effective_from": "2024-01-01", "calories": 2000, "protein": 150, "exercise_adds_to_budget": true})).await;
    let e1 = t.post(&format!("/api/users/{a}/entries"), json!({"date": "2024-01-03", "meal": "breakfast", "food_id": milk, "quantity": 2, "unit": "cup"})).await;
    assert!(approx(&e1["nutrients"]["calories"], 297.6));
    assert_eq!(e1["display_quantity"], "2 cup (480 ml)");
    let e2 = t.post(&format!("/api/users/{a}/entries"), json!({"date": "2024-01-03", "meal": "Breakfast", "food_id": oats, "quantity": 50, "unit": "grams"})).await;
    assert!(approx(&e2["nutrients"]["calories"], 189.5));
    t.post(&format!("/api/users/{a}/entries"), json!({"date": "2024-01-03", "meal": "dinner", "name": "Pizza night", "nutrients": {"calories": 900, "protein": 40}})).await;
    t.post(
        &format!("/api/users/{a}/exercises"),
        json!({"date": "2024-01-03", "name": "Run", "calories": 300}),
    )
    .await;
    let d = t
        .get(&format!("/api/users/{a}/diary?date=2024-01-03"))
        .await;
    assert_eq!(d["entries"].as_array().unwrap().len(), 3);
    assert!(approx(&d["budget"]["eaten"], 1387.1));
    assert_eq!(d["budget"]["budget"], 2300.0, "exercise adds to budget");
    assert!(approx(&d["budget"]["remaining"], 912.9));
    assert!(approx(&d["meals"]["breakfast"]["calories"], 487.1));
    assert_eq!(d["coverage"]["calcium"], 0.333);
    // other user is isolated
    let d2 = t
        .get(&format!("/api/users/{b}/diary?date=2024-01-03"))
        .await;
    assert_eq!(d2["entries"].as_array().unwrap().len(), 0);
    assert!(d2["budget"]["budget"].is_null());
    // edit and delete
    let id = e1["id"].as_i64().unwrap();
    let e = t
        .put(&format!("/api/entries/{id}"), json!({"quantity": 1}))
        .await;
    assert!(approx(&e["nutrients"]["calories"], 148.8));
    assert_eq!(e["unit"], "cup", "unit is kept when only quantity changes");
    let e = t
        .put(
            &format!("/api/entries/{id}"),
            json!({"quantity": 100, "unit": "ml"}),
        )
        .await;
    assert!(approx(&e["nutrients"]["calories"], 62.0));
    let (s, v) = t
        .req(
            "PUT",
            &format!("/api/entries/{id}"),
            Some(json!({"unit": "g"})),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "grams of a volume food: {v}");
    let (s, _) = t.req("DELETE", &format!("/api/entries/{id}"), None).await;
    assert!(s.is_success());
    // week balance only counts days before the selected one
    t.post(
        &format!("/api/users/{a}/entries"),
        json!({"date": "2024-01-02", "name": "Day 2", "nutrients": {"calories": 1500}}),
    )
    .await;
    let d = t
        .get(&format!("/api/users/{a}/diary?date=2024-01-03"))
        .await;
    assert_eq!(d["week"]["start"], "2024-01-01", "weeks start on Monday");
    assert_eq!(d["week"]["balance"], 500.0);
    // Streak: Jan 2 (1500 of 2000) is on budget, Jan 1 has no entries, so the run is 1 day.
    assert_eq!(d["streak"]["on_budget"], 1);
    assert_eq!(d["streak"]["logged"], 1);
    // BUG-7 (bug bash): an entry whose serving was deleted can still be edited (scaled as logged).
    let glass_food = t
        .post(
            "/api/foods",
            json!({"name": "Juice", "basis_unit": "ml", "nutrients": {"calories": 45},
            "servings": [{"label": "cup", "amount": 240}, {"label": "glass", "amount": 300}]}),
        )
        .await["food"]
        .clone();
    let gid = glass_food["id"].as_i64().unwrap();
    let ge = t
        .post(
            &format!("/api/users/{a}/entries"),
            json!({"date": "2024-01-05", "food_id": gid, "quantity": 1, "unit": "glass"}),
        )
        .await;
    t.put(
        &format!("/api/foods/{gid}"),
        json!({"name": "Juice", "basis_unit": "ml", "nutrients": {"calories": 45},
        "servings": [{"label": "cup", "amount": 240}]}),
    )
    .await;
    let ge2 = t
        .put(
            &format!("/api/entries/{}", ge["id"]),
            json!({"quantity": 2, "meal": "snacks"}),
        )
        .await;
    assert!(approx(&ge2["nutrients"]["calories"], 270.0), "{ge2}");
    assert_eq!(ge2["unit"], "glass");
    assert_eq!(ge2["meal"], "snacks");

    // copy a meal to another user's diary
    let copied = t.post(&format!("/api/users/{b}/copy"), json!({"from_date": "2024-01-03", "to_date": "2024-01-04", "meal": "dinner", "from_user_id": a})).await;
    assert_eq!(copied.as_array().unwrap().len(), 1);
    assert_eq!(copied[0]["name"], "Pizza night");
    // bad input
    let (s, _) = t
        .req(
            "POST",
            &format!("/api/users/{a}/entries"),
            Some(json!({"food_id": milk, "quantity": -1})),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = t
        .req(
            "POST",
            &format!("/api/users/{a}/entries"),
            Some(json!({"food_id": milk, "meal": "brunch"})),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = t
        .req(
            "GET",
            &format!("/api/users/{a}/diary?date=2024-02-30"),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn shared_recipes_recompute_and_reject_cycles() {
    let t = T::new().await;
    let (a, b) = two_users(&t).await;
    let (oats, milk, banana) = oats_milk_banana(&t).await;
    let r = t
        .post(
            "/api/foods",
            json!({"name": "Morning oats", "kind": "recipe", "recipe_yield": 1, "ingredients": [
            {"food_id": oats, "quantity": 50, "unit": "g"},
            {"food_id": milk, "quantity": 1, "unit": "cup"},
            {"food_id": banana, "quantity": 1, "unit": "medium"}]}),
        )
        .await;
    let rid = r["food"]["id"].as_i64().unwrap();
    // 189.5 + 148.8 + 105.02
    assert!(
        approx(&r["food"]["nutrients"]["calories"], 443.32),
        "{}",
        r["food"]["nutrients"]
    );
    assert_eq!(r["food"]["ingredients"].as_array().unwrap().len(), 3);
    for u in [a, b] {
        t.post(
            &format!("/api/users/{u}/entries"),
            json!({"date": "2024-03-01", "food_id": rid, "quantity": 1}),
        )
        .await;
    }
    let ea = t
        .get(&format!("/api/users/{a}/diary?date=2024-03-01"))
        .await;
    assert!(approx(&ea["totals"]["calories"], 443.32));
    let eb = t
        .get(&format!("/api/users/{b}/diary?date=2024-03-01"))
        .await;
    assert!(approx(
        &eb["totals"]["protein"],
        ea["totals"]["protein"].as_f64().unwrap()
    ));

    // Changing milk updates the recipe and both diaries.
    let mut milk_food = t.get(&format!("/api/foods/{milk}")).await;
    milk_food["nutrients"]["calories"] = json!(42);
    let input = json!({"name": milk_food["name"], "brand": milk_food["brand"], "basis_amount": 100, "basis_unit": "ml",
        "nutrients": milk_food["nutrients"], "servings": [{"label": "cup", "amount": 240}]});
    let upd = t.put(&format!("/api/foods/{milk}"), input).await;
    assert_eq!(upd["entries_updated"], 2);
    let ea = t
        .get(&format!("/api/users/{a}/diary?date=2024-03-01"))
        .await;
    assert!(
        approx(&ea["totals"]["calories"], 443.32 - 48.0),
        "{}",
        ea["totals"]["calories"]
    );

    // A recipe cannot contain itself (directly or through another recipe).
    let outer = t.post("/api/foods", json!({"name": "Big breakfast", "kind": "recipe", "ingredients": [{"food_id": rid, "quantity": 1}]})).await;
    let oid = outer["food"]["id"].as_i64().unwrap();
    let (s, v) = t.req("PUT", &format!("/api/foods/{rid}"), Some(json!({"name": "Morning oats", "kind": "recipe", "ingredients": [{"food_id": oid, "quantity": 1}]}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");

    // Cooked weight makes the recipe loggable in grams.
    let chili = t
        .post(
            "/api/foods",
            json!({"name": "Chili", "kind": "recipe", "recipe_yield": 4, "cooked_weight_g": 1000,
            "ingredients": [{"food_id": oats, "quantity": 400, "unit": "g"}]}),
        )
        .await;
    let cid = chili["food"]["id"].as_i64().unwrap();
    assert_eq!(chili["food"]["basis_unit"], "g");
    let e = t
        .post(
            &format!("/api/users/{a}/entries"),
            json!({"date": "2024-03-01", "food_id": cid, "quantity": 250, "unit": "g"}),
        )
        .await;
    let e2 = t
        .post(
            &format!("/api/users/{a}/entries"),
            json!({"date": "2024-03-01", "food_id": cid, "quantity": 1, "unit": "serving"}),
        )
        .await;
    assert!(approx(&e["nutrients"]["calories"], 379.0));
    assert_eq!(e["nutrients"]["calories"], e2["nutrients"]["calories"]);

    // Deleting a used food archives it; the recipe still works.
    let d = t
        .req("DELETE", &format!("/api/foods/{banana}"), None)
        .await
        .1;
    assert_eq!(d["archived"], true);
    assert!(t
        .get("/api/foods?q=banana")
        .await
        .as_array()
        .unwrap()
        .is_empty());
    let r = t.get(&format!("/api/foods/{rid}")).await;
    assert_eq!(r["ingredients"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn goals_are_effective_dated_and_weights_trend() {
    let t = T::new().await;
    let (a, b) = two_users(&t).await;
    t.post(
        &format!("/api/users/{a}/goals"),
        json!({"effective_from": "2024-01-01", "calories": 2000, "protein": 150}),
    )
    .await;
    t.post(
        &format!("/api/users/{a}/goals"),
        json!({"effective_from": "2024-02-01", "calories": 1800}),
    )
    .await;
    let goals = t.get(&format!("/api/users/{a}/goals")).await;
    assert_eq!(goals.as_array().unwrap().len(), 2);
    assert_eq!(goals[1]["protein"], 150.0, "unspecified fields carry over");
    let d = t
        .get(&format!("/api/users/{a}/diary?date=2024-01-15"))
        .await;
    assert_eq!(d["budget"]["goal"], 2000.0);
    let d = t
        .get(&format!("/api/users/{a}/diary?date=2024-02-15"))
        .await;
    assert_eq!(d["budget"]["goal"], 1800.0);
    let d = t
        .get(&format!("/api/users/{a}/diary?date=2023-12-15"))
        .await;
    assert_eq!(
        d["budget"]["goal"], 2000.0,
        "days before the first goal use the first goal"
    );

    // weights: lb users enter pounds, storage is kg, one per day
    let w = t
        .post(
            &format!("/api/users/{b}/weights"),
            json!({"date": "2024-01-01", "weight": 150}),
        )
        .await;
    assert!(approx(&w["weight_kg"], 68.039));
    t.post(
        &format!("/api/users/{b}/weights"),
        json!({"date": "2024-01-01", "weight": 149}),
    )
    .await;
    t.post(
        &format!("/api/users/{b}/weights"),
        json!({"date": "2024-01-02", "weight": 68, "unit": "kg"}),
    )
    .await;
    let ws = t.get(&format!("/api/users/{b}/weights")).await;
    assert_eq!(ws.as_array().unwrap().len(), 2, "same day replaces");
    let (w0, t1) = (
        ws[0]["weight_kg"].as_f64().unwrap(),
        ws[1]["trend_kg"].as_f64().unwrap(),
    );
    assert!(
        t1 > w0 && t1 < 68.0,
        "trend moves 10% toward the new weight: {w0} -> {t1}"
    );
    let (s, _) = t
        .req(
            "POST",
            &format!("/api/users/{b}/weights"),
            Some(json!({"weight": 5})),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // BUG-8 (bug bash): no weigh-ins from the future.
    let (s, v) = t
        .req(
            "POST",
            &format!("/api/users/{b}/weights"),
            Some(json!({"weight": 150, "date": "2099-01-01"})),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
}

#[tokio::test]
async fn summary_stats() {
    let t = T::new().await;
    let (a, _) = two_users(&t).await;
    t.post(
        &format!("/api/users/{a}/goals"),
        json!({"effective_from": "2024-01-01", "calories": 2000, "protein": 100}),
    )
    .await;
    for (i, (cal, p)) in [(1800, 120), (2200, 90), (1900, 100), (2000, 110)]
        .iter()
        .enumerate()
    {
        t.post(&format!("/api/users/{a}/entries"), json!({"date": format!("2024-01-0{}", i + 1), "name": "Food", "nutrients": {"calories": cal, "protein": p}})).await;
    }
    for (d, kg) in [("2024-01-01", 80.0), ("2024-01-04", 79.6)] {
        t.post(
            &format!("/api/users/{a}/weights"),
            json!({"date": d, "weight": kg, "unit": "kg"}),
        )
        .await;
    }
    let s = t
        .get(&format!(
            "/api/users/{a}/summary?from=2024-01-01&to=2024-01-05"
        ))
        .await;
    let st = &s["stats"];
    assert_eq!(st["logged_days"], 4);
    assert_eq!(st["avg_calories"], 1975.0);
    assert_eq!(st["within_budget_days"], 3);
    assert_eq!(st["balance"], 100.0);
    assert_eq!(st["protein_hit_days"], 3);
    assert_eq!(st["protein_hit_rate"], 0.75);
    assert_eq!(
        st["protein_streak"], 0,
        "Jan 5 is not logged, so the streak is broken"
    );
    assert_eq!(s["days"].as_array().unwrap().len(), 5);
    assert!(
        s["days"][2]["trend_kg"].as_f64().is_some(),
        "trend carries forward between weigh-ins"
    );
    let s = t
        .get(&format!(
            "/api/users/{a}/summary?from=2024-01-01&to=2024-01-04"
        ))
        .await;
    assert_eq!(s["stats"]["protein_streak"], 2);
    let (st, _) = t
        .req(
            "GET",
            &format!("/api/users/{a}/summary?from=2024-01-05&to=2024-01-01"),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn external_search_import_and_barcode() {
    let t = T::new().await;
    two_users(&t).await;
    let r = t
        .get("/api/external/search?q=costco%20milk&source=usda")
        .await;
    let first = &r["results"][0];
    assert_eq!(first["source"], "usda");
    assert!(r["results"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["source_id"] == "2460407"));
    let imp = t
        .post(
            "/api/external/import",
            json!({"source": "usda", "id": "2460407", "aliases": "costco milk"}),
        )
        .await;
    assert_eq!(imp["created"], true);
    assert_eq!(imp["food"]["name"], "Organic Whole Milk");
    assert_eq!(imp["food"]["brand"], "Kirkland Signature");
    assert_eq!(imp["food"]["servings"][0]["label"], "cup");
    let fid = imp["food"]["id"].clone();
    let again = t
        .post(
            "/api/external/import",
            json!({"source": "usda", "id": "2460407"}),
        )
        .await;
    assert_eq!(again["created"], false);
    assert_eq!(again["food"]["id"], fid);
    let r = t
        .get("/api/external/search?q=costco%20milk&source=usda")
        .await;
    let c = r["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["source_id"] == "2460407")
        .unwrap()
        .clone();
    assert_eq!(c["in_library"], fid);
    assert_eq!(t.get("/api/foods?q=costco%20milk").await[0]["id"], fid);

    // BUG-2 (bug bash): Open Food Facts hits named only "Kirkland" are useless and must not be offered.
    let r = t
        .get("/api/external/search?q=costco%20milk&source=off")
        .await;
    for c in r["results"].as_array().unwrap() {
        assert_ne!(
            c["name"].as_str().unwrap().to_lowercase(),
            c["brand"].as_str().unwrap_or("").to_lowercase(),
            "{c}"
        );
    }

    // barcode: local hit (USDA item carries its UPC), then OFF
    let b = t.get("/api/external/barcode/096619163014").await;
    assert_eq!(b["food"]["id"], fid);
    let b = t.get("/api/external/barcode/3017624010701").await;
    assert_eq!(b["candidate"]["name"], "Nutella");
    let imp = t
        .post(
            "/api/external/import",
            json!({"source": "off", "id": "3017624010701"}),
        )
        .await;
    assert!(approx(&imp["food"]["nutrients"]["calories"], 539.0));
    let b = t.get("/api/external/barcode/1234567890123").await;
    assert!(b["food"].is_null() && b["candidate"].is_null());
    let (s, _) = t.req("GET", "/api/external/barcode/12", None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // generic USDA food with portions
    let imp = t
        .post(
            "/api/external/import",
            json!({"source": "usda", "id": "173944"}),
        )
        .await;
    assert!(imp["food"]["servings"].as_array().unwrap().len() > 2);
    assert!(imp["food"]["nutrients"]["vitamin_b6"].as_f64().is_some());
}

#[tokio::test]
async fn usda_transient_400_is_retried_and_fndds_servings_are_named() {
    let t = T::new().await;
    two_users(&t).await;
    // The mock fails the first request for this id the way USDA's nginx does.
    let imp = t
        .post(
            "/api/external/import",
            json!({"source": "usda", "id": "2709223"}),
        )
        .await;
    assert_eq!(imp["food"]["name"], "Avocado, raw");
    let labels: Vec<&str> = imp["food"]["servings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["label"].as_str().unwrap())
        .collect();
    assert!(!labels.contains(&"90000"), "{labels:?}");
    assert_eq!(imp["food"]["servings"][0]["label"], "slice");
}

#[tokio::test]
async fn mcp_protocol() {
    let t = T::new().await;
    let v = t.mcp("initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}})).await;
    assert_eq!(v["result"]["protocolVersion"], "2025-03-26");
    assert!(v["result"]["capabilities"]["tools"].is_object());
    assert!(v["result"]["instructions"]
        .as_str()
        .unwrap()
        .contains("search_foods"));
    let v = t
        .mcp("initialize", json!({"protocolVersion": "1999-01-01"}))
        .await;
    assert_eq!(v["result"]["protocolVersion"], "2025-06-18");
    let (s, _) = t
        .req(
            "POST",
            "/mcp",
            Some(json!({"jsonrpc": "2.0", "method": "notifications/initialized"})),
        )
        .await;
    assert_eq!(s, StatusCode::ACCEPTED);
    let tools = t.mcp("tools/list", json!({})).await;
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for n in [
        "search_foods",
        "create_food",
        "create_recipe",
        "log_food",
        "quick_add",
        "get_diary",
        "log_weight",
        "get_summary",
        "set_goals",
        "search_online_foods",
    ] {
        assert!(names.contains(&n), "missing {n}");
    }
    let v = t
        .mcp("tools/call", json!({"name": "nope", "arguments": {}}))
        .await;
    assert_eq!(v["error"]["code"], -32602);
    let v = t.mcp("bogus/method", json!({})).await;
    assert_eq!(v["error"]["code"], -32601);
    let (s, v) = t.req("POST", "/mcp", Some(json!([{"jsonrpc": "2.0", "id": 1, "method": "ping"}, {"jsonrpc": "2.0", "id": 2, "method": "ping"}]))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v.as_array().unwrap().len(), 2);
    let (s, _) = t.req("GET", "/mcp", None).await;
    assert_eq!(s, StatusCode::METHOD_NOT_ALLOWED);
    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json")
        .header("origin", "http://evil.example")
        .header("host", "192.168.1.5:8484")
        .body(Body::from(
            json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}).to_string(),
        ))
        .unwrap();
    assert_eq!(
        t.app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let req = Request::builder()
        .method("POST")
        .uri("/mcp")
        .body(Body::from("{not json"))
        .unwrap();
    assert_eq!(
        t.app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn mcp_ai_workflow() {
    let t = T::new().await;
    // No users yet: user tools explain what to do.
    let (err, v) = t.tool("get_diary", json!({})).await;
    assert!(err && v.as_str().unwrap().contains("no users"));
    two_users(&t).await;
    let (err, v) = t.tool("get_diary", json!({})).await;
    assert!(
        err && v.as_str().unwrap().contains("Jordan"),
        "asks which user: {v}"
    );

    // Library miss -> online search -> import -> log.
    let s = t
        .ok_tool("search_foods", json!({"query": "costco milk"}))
        .await;
    assert_eq!(s["count"], 0);
    let o = t
        .ok_tool(
            "search_online_foods",
            json!({"query": "costco milk", "source": "usda"}),
        )
        .await;
    assert!(o["results"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"] == "2460407"));
    let imp = t
        .ok_tool(
            "import_online_food",
            json!({"source": "usda", "id": "2460407", "aliases": "costco milk"}),
        )
        .await;
    let milk = imp["food"]["id"].as_i64().unwrap();
    let logged = t.ok_tool("log_food", json!({"user": "jordan", "food_id": milk, "quantity": "1", "unit": "cups", "meal": "breakfast", "date": "2024-05-01"})).await;
    assert!(approx(&logged["logged"]["nutrients"]["calories"], 148.8));
    assert_eq!(logged["day"]["date"], "2024-05-01");

    // Nothing online: create a food from a label, with servings, then log by serving label.
    let (err, v) = t.tool("create_food", json!({"name": "Protein bar", "nutrients": {"kcal": 200, "proteins": 20, "carbohydrate": 22, "fat": 7, "protien": 1}})).await;
    assert!(
        err && v.as_str().unwrap().contains("protien"),
        "unknown nutrient keys are reported: {v}"
    );
    let bar = t
        .ok_tool("create_food", json!({"name": "Protein bar", "brand": "Kirkland Signature", "basis_amount": 1, "basis_unit": "serving",
            "nutrients": {"kcal": 190, "proteins": 21, "carbohydrate": 22, "fat": 7, "fiber": 15, "sodium": 135},
            "servings": [{"label": "bar", "amount": 1}], "aliases": "costco protein bar"}))
        .await;
    let bar_id = bar["food"]["id"].as_i64().unwrap();
    assert_eq!(bar["food"]["source"], "ai");
    let l = t.ok_tool("log_food", json!({"user": "Riley", "food_id": bar_id, "quantity": 2, "unit": "bar", "date": "2024-05-01", "meal": "snacks"})).await;
    assert!(approx(&l["logged"]["nutrients"]["protein"], 42.0));
    let (err, v) = t
        .tool(
            "log_food",
            json!({"user": "Riley", "food_id": bar_id, "unit": "cup"}),
        )
        .await;
    assert!(
        err && v.as_str().unwrap().contains("Options"),
        "unit errors list options: {v}"
    );

    // Recipe shared between users.
    let oats = t.ok_tool("create_food", json!({"name": "Rolled oats", "nutrients": {"calories": 379, "protein": 13.2, "carbs": 67.7, "fat": 6.5}})).await["food"]["id"].as_i64().unwrap();
    let r = t
        .ok_tool("create_recipe", json!({"name": "Morning oats", "ingredients": [{"food_id": oats, "quantity": 50, "unit": "g"}, {"food_id": milk, "quantity": 1, "unit": "cup"}]}))
        .await;
    let rid = r["recipe"]["id"].as_i64().unwrap();
    assert!(approx(&r["recipe"]["per_basis"]["calories"], 338.3));
    t.ok_tool(
        "log_food",
        json!({"user": "Riley", "food_id": rid, "date": "2024-05-01", "meal": "breakfast"}),
    )
    .await;
    let found = t
        .ok_tool("search_foods", json!({"query": "oats", "kind": "recipe"}))
        .await;
    assert_eq!(found["foods"][0]["id"], rid);

    // Quick add, diary, update/delete entry.
    let q = t.ok_tool("quick_add", json!({"user": "Jordan", "name": "Restaurant curry", "nutrients": {"calories": 850, "protein": 35}, "meal": "dinner", "date": "2024-05-01"})).await;
    let qid = q["logged"]["id"].as_i64().unwrap();
    let u = t
        .ok_tool("update_entry", json!({"entry_id": qid, "quantity": 0.5}))
        .await;
    assert!(approx(&u["entry"]["nutrients"]["calories"], 425.0));
    let d = t
        .ok_tool(
            "get_diary",
            json!({"user": "Jordan", "date": "2024-05-01", "include_micros": true}),
        )
        .await;
    assert_eq!(d["entries"].as_array().unwrap().len(), 2);
    assert!(approx(&d["eaten"]["calories"], 573.8));
    assert!(d["micros"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["key"] == "calcium"));
    t.ok_tool("delete_entry", json!({"entry_id": qid})).await;

    // Update food: nutrients merge, past entries recalculated.
    let up = t
        .ok_tool(
            "update_food",
            json!({"food_id": bar_id, "nutrients": {"sugar": 1}}),
        )
        .await;
    assert_eq!(up["past_entries_updated"], 1);
    let f = t.ok_tool("get_food", json!({"food_id": bar_id})).await;
    assert_eq!(
        f["nutrients"]["protein"], 21.0,
        "untouched nutrients are kept"
    );
    assert_eq!(f["nutrients"]["sugar"], 1.0);

    // Goals, weight, summary, default user via URL.
    let g = t.ok_tool("set_goals", json!({"user": "Jordan", "calories": 2200, "protein": 160, "effective_from": "2024-04-01"})).await;
    assert_eq!(g["goal"]["calories"], 2200.0);
    let g = t
        .ok_tool(
            "set_goals",
            json!({"user": "Jordan", "protein": 170, "effective_from": "2024-04-20"}),
        )
        .await;
    assert_eq!(
        g["goal"]["calories"], 2200.0,
        "partial update keeps calories"
    );
    let w = t
        .ok_tool(
            "log_weight",
            json!({"user": "Riley", "weight": 140, "date": "2024-05-01"}),
        )
        .await;
    assert_eq!(w["display"], "140.0 lb");
    // BUG-10 (bug bash): a weight with a unit in the string is understood.
    let w = t
        .ok_tool(
            "log_weight",
            json!({"user": "Riley", "weight": "64kg", "date": "2024-05-02"}),
        )
        .await;
    assert!(approx(&w["weight"]["weight_kg"], 64.0), "{w}");
    let s = t.ok_tool("get_summary", json!({"user": "Jordan", "from": "2024-04-25", "to": "2024-05-02", "include_days": true})).await;
    assert_eq!(s["stats"]["logged_days"], 1);
    assert_eq!(
        s["days"].as_array().unwrap().len(),
        2,
        "starts at the first log (05-01), not 04-25"
    );
    let (s, v) = t
        .req("POST", "/mcp?user=Riley", Some(json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": {"name": "get_diary", "arguments": {"date": "2024-05-01"}}})))
        .await;
    assert_eq!(s, StatusCode::OK);
    let d: Value =
        serde_json::from_str(v["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(d["user"], "Riley");
    assert_eq!(d["entries"].as_array().unwrap().len(), 2);
    let c = t.ok_tool("copy_meal", json!({"user": "Jordan", "from_user": "Riley", "from_date": "2024-05-01", "to_date": "2024-05-02", "meal": "breakfast"})).await;
    assert_eq!(c["copied"].as_array().unwrap().len(), 1);
    let (err, v) = t
        .tool(
            "log_food",
            json!({"user": "Jordan", "food_id": bar_id, "bogus": 1}),
        )
        .await;
    assert!(err && v.as_str().unwrap().contains("bogus"));
}

#[tokio::test]
async fn meal_times_occasions_and_usual_times() {
    let t = T::new().await;
    let (a, b) = two_users(&t).await;
    let (oats, milk, banana) = oats_milk_banana(&t).await;
    let log = |date: &str, meal: &str, food: i64, time: Option<&str>| {
        let mut v = json!({"date": date, "meal": meal, "food_id": food, "quantity": 1});
        if let Some(tm) = time {
            v["time"] = json!(tm);
        }
        v
    };
    // Breakfast in one sitting (two entries 10 minutes apart), lunch, an afternoon snack.
    t.post(
        &format!("/api/users/{a}/entries"),
        log("2024-06-10", "breakfast", oats, Some("7:40")),
    )
    .await;
    t.post(
        &format!("/api/users/{a}/entries"),
        log("2024-06-10", "breakfast", milk, Some("7:50 am")),
    )
    .await;
    t.post(
        &format!("/api/users/{a}/entries"),
        log("2024-06-10", "lunch", banana, Some("12:30")),
    )
    .await;
    let snack = t
        .post(
            &format!("/api/users/{a}/entries"),
            log("2024-06-10", "snacks", banana, Some("3:10 pm")),
        )
        .await;
    assert_eq!(snack["time"], "15:10");
    t.post(
        &format!("/api/users/{a}/entries"),
        log("2024-06-10", "dinner", milk, None),
    )
    .await;
    // Dinner around 19:30 on the three days before.
    for (d, tm) in [
        ("2024-06-07", "19:20"),
        ("2024-06-08", "19:30"),
        ("2024-06-09", "19:40"),
    ] {
        t.post(
            &format!("/api/users/{a}/entries"),
            log(d, "dinner", milk, Some(tm)),
        )
        .await;
    }
    let d = t
        .get(&format!("/api/users/{a}/diary?date=2024-06-10"))
        .await;
    let occ = d["timing"]["occasions"].as_array().unwrap();
    assert_eq!(occ.len(), 3, "{occ:?}");
    assert_eq!(occ[0]["meal"], "breakfast");
    assert_eq!(occ[0]["time"], "07:40");
    assert_eq!(occ[0]["entries"], 2);
    assert!(
        approx(&occ[0]["calories"], 300.4),
        "1/2 cup oats (151.6) + 1 cup milk (148.8)"
    );
    assert_eq!(occ[2]["time"], "15:10");
    assert_eq!(d["timing"]["first"], "07:40");
    assert_eq!(d["timing"]["last"], "15:10");
    assert_eq!(
        d["timing"]["untimed"], 1,
        "the past-day dinner without a time stays unknown"
    );
    assert_eq!(d["timing"]["usual"]["dinner"], "19:30");
    assert!(d["timing"]["usual"]["breakfast"].is_null(), "needs 3 days");
    // Entries are ordered by time within a meal.
    assert_eq!(d["entries"][0]["time"], "07:40");

    // Edit and clear a time; reject nonsense.
    let id = snack["id"].as_i64().unwrap();
    let e = t
        .put(&format!("/api/entries/{id}"), json!({"time": "16:05"}))
        .await;
    assert_eq!(e["time"], "16:05");
    let e = t
        .put(&format!("/api/entries/{id}"), json!({"quantity": 2}))
        .await;
    assert_eq!(e["time"], "16:05", "untouched by other edits");
    let e = t
        .put(&format!("/api/entries/{id}"), json!({"time": null}))
        .await;
    assert!(e["time"].is_null());
    let (s, v) = t
        .req(
            "PUT",
            &format!("/api/entries/{id}"),
            Some(json!({"time": "25:99"})),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");

    // Copies keep the time they were eaten at.
    let c = t.post(&format!("/api/users/{b}/copy"), json!({"from_date": "2024-06-10", "to_date": "2024-06-11", "meal": "breakfast", "from_user_id": a})).await;
    assert_eq!(c[0]["time"], "07:40");

    // Today without a time: "now" when the meal fits the hour, unknown for a back-filled meal.
    let now = chrono::Local::now();
    let fits = calorie_man::service::diary::meal_for_hour(chrono::Timelike::hour(&now));
    let other = if fits == "breakfast" {
        "dinner"
    } else {
        "breakfast"
    };
    let today = now.format("%Y-%m-%d").to_string();
    let e1 = t
        .post(
            &format!("/api/users/{a}/entries"),
            log(&today, fits, banana, None),
        )
        .await;
    assert!(e1["time"].as_str().unwrap().len() == 5, "{e1}");
    let e2 = t
        .post(
            &format!("/api/users/{a}/entries"),
            log(&today, other, banana, None),
        )
        .await;
    assert!(e2["time"].is_null(), "{e2}");

    // MCP: time words are understood.
    let l = t.ok_tool("log_food", json!({"user": "Jordan", "food_id": milk, "date": "2024-06-12", "meal": "dinner", "time": "7:40 pm"})).await;
    assert_eq!(l["logged"]["time"], "19:40");
    let g = t
        .ok_tool("get_diary", json!({"user": "Jordan", "date": "2024-06-10"}))
        .await;
    assert_eq!(g["usual_meal_times"]["dinner"], "19:30");
    assert_eq!(g["eating_window"]["first"], "07:40");
    let (err, v) = t
        .tool(
            "log_food",
            json!({"user": "Jordan", "food_id": milk, "time": "teatime"}),
        )
        .await;
    assert!(err && v.as_str().unwrap().contains("invalid time"), "{v}");
}
