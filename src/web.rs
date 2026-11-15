use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

/// The built Svelte app. In debug builds files are read from disk, so `npm run build` is enough to
/// see UI changes without recompiling.
#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct Assets;

pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.starts_with("api/") || path == "mcp" {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    if !path.is_empty() {
        if let Some(f) = Assets::get(path) {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            let cache = if path.starts_with("assets/") {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            return (
                [
                    (header::CONTENT_TYPE, mime.as_ref().to_string()),
                    (header::CACHE_CONTROL, cache.to_string()),
                ],
                f.data,
            )
                .into_response();
        }
        // A missing file with an extension is a real 404, not an app route.
        if path
            .rsplit('/')
            .next()
            .map(|s| s.contains('.'))
            .unwrap_or(false)
        {
            return (StatusCode::NOT_FOUND, "not found").into_response();
        }
    }
    match Assets::get("index.html") {
        Some(f) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-cache")], f.data).into_response(),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "The web UI is not built. Run `npm ci && npm run build` in web/, then restart (or rebuild for release).",
        )
            .into_response(),
    }
}
