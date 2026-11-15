pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod external;
pub mod mcp;
pub mod nutrients;
pub mod seed;
pub mod service;
pub mod units;
pub mod util;
pub mod web;

use std::sync::Arc;

use axum::Router;
use config::Config;
use db::Pool;
use error::{AppError, AppResult};
use rusqlite::Connection;

#[derive(Clone)]
pub struct AppState {
    pub pool: Pool,
    pub cfg: Arc<Config>,
    pub http: reqwest::Client,
}

impl AppState {
    pub fn new(cfg: Config) -> anyhow::Result<Self> {
        let pool = db::open_pool(&cfg.db_path)?;
        let http = reqwest::Client::builder()
            .user_agent(cfg.user_agent.clone())
            .timeout(std::time::Duration::from_secs(20))
            .build()?;
        Ok(AppState {
            pool,
            cfg: Arc::new(cfg),
            http,
        })
    }

    /// Run blocking SQLite work on the blocking thread pool.
    pub async fn db<F, T>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&mut Connection) -> AppResult<T> + Send + 'static,
        T: Send + 'static,
    {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut c = pool.get().map_err(AppError::from)?;
            f(&mut c)
        })
        .await?
    }
}

pub fn app(state: AppState) -> Router {
    use tower_http::compression::CompressionLayer;
    use tower_http::trace::TraceLayer;
    Router::new()
        .nest("/api", api::router())
        .merge(mcp::router())
        .fallback(web::static_handler)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
