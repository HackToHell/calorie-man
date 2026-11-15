//! Calorie Man in the browser, for the GitHub Pages demo.
//!
//! The page's `fetch` calls to `/api/...` and `/mcp` are routed here instead of to a server. They are
//! answered by the same axum router, services and SQLite schema as the real app, on a database that
//! lives in memory and starts over on every reload.

use std::cell::RefCell;
use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use calorie_man::config::Config;
use calorie_man::AppState;
use http_body_util::BodyExt;
use tower::ServiceExt;
use wasm_bindgen::prelude::*;

thread_local! {
    static APP: RefCell<Option<Router>> = const { RefCell::new(None) };
}

/// Create the in-memory database and fill it with `days` of demo history for two people.
#[wasm_bindgen]
pub fn start(days: u32) -> Result<(), JsError> {
    let mut cfg = Config::new(PathBuf::new());
    cfg.off_text_search = false;
    let state = AppState::in_memory(cfg).map_err(|e| JsError::new(&e.to_string()))?;
    {
        let mut c = state
            .pool
            .lock()
            .map_err(|_| JsError::new("database lock poisoned"))?;
        calorie_man::seed::seed_demo(&mut c, days as i64, chrono::Local::now().date_naive())
            .map_err(|e| JsError::new(&e.to_string()))?;
    }
    let app = calorie_man::app(state);
    APP.with(|a| *a.borrow_mut() = Some(app));
    Ok(())
}

/// Answer one HTTP request. `headers` is a flat `[name, value, name, value, ...]` list; the result is
/// `{ status, headers: [name, value, ...], body: Uint8Array }`.
#[wasm_bindgen]
pub async fn handle(
    method: String,
    path: String,
    headers: Vec<String>,
    body: Option<Vec<u8>>,
) -> Result<JsValue, JsError> {
    let app = APP
        .with(|a| a.borrow().clone())
        .ok_or_else(|| JsError::new("call start() first"))?;
    let mut req = Request::builder().method(method.as_str()).uri(&path);
    for pair in headers.chunks(2) {
        if let [k, v] = pair {
            req = req.header(k.as_str(), v.as_str());
        }
    }
    let req = req
        .body(Body::from(body.unwrap_or_default()))
        .map_err(|e| JsError::new(&e.to_string()))?;
    let resp = match app.oneshot(req).await {
        Ok(r) => r,
        Err(never) => match never {},
    };
    let status = resp.status();
    let mut out_headers = vec![];
    for (k, v) in resp.headers() {
        if let Ok(v) = v.to_str() {
            out_headers.push(k.as_str().to_string());
            out_headers.push(v.to_string());
        }
    }
    let bytes = resp
        .into_body()
        .collect()
        .await
        .map_err(|e| JsError::new(&e.to_string()))?
        .to_bytes();
    let out = js_sys::Object::new();
    let set = |k: &str, v: JsValue| js_sys::Reflect::set(&out, &JsValue::from_str(k), &v);
    set("status", JsValue::from(status.as_u16())).ok();
    set(
        "headers",
        out_headers
            .into_iter()
            .map(JsValue::from)
            .collect::<js_sys::Array>()
            .into(),
    )
    .ok();
    // Status codes like 204 must not carry a body in a JS Response.
    let body = if status == StatusCode::NO_CONTENT || status == StatusCode::NOT_MODIFIED {
        JsValue::NULL
    } else {
        js_sys::Uint8Array::from(bytes.as_ref()).into()
    };
    set("body", body).ok();
    Ok(out.into())
}
