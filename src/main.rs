use calorie_man::config::Config;
use calorie_man::AppState;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Parser)]
#[command(
    name = "calorie-man",
    version,
    about = "Household calorie & nutrition tracker: web UI, JSON API and MCP server"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the web server (default).
    Serve {
        #[arg(long, env = "CALORIE_MAN_DB", default_value = "data/calorie-man.db")]
        db: PathBuf,
        /// Address to listen on. 0.0.0.0 makes it reachable from other devices on the network.
        #[arg(long, env = "CALORIE_MAN_BIND", default_value = "0.0.0.0:8484")]
        bind: String,
    },
    /// Bridge stdio to the HTTP MCP endpoint, for MCP clients that only launch local commands.
    McpStdio {
        /// e.g. http://192.168.1.20:8484/mcp?user=alex
        #[arg(
            long,
            env = "CALORIE_MAN_MCP_URL",
            default_value = "http://127.0.0.1:8484/mcp"
        )]
        url: String,
    },
    /// Fill a database with two demo users, a food library and N days of history.
    SeedDemo {
        #[arg(long, env = "CALORIE_MAN_DB", default_value = "data/demo.db")]
        db: PathBuf,
        #[arg(long, default_value_t = 60)]
        days: i64,
        /// Delete the database file first and start over.
        #[arg(long)]
        force: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cmd = cli.cmd.unwrap_or(Cmd::Serve {
        db: std::env::var("CALORIE_MAN_DB")
            .map(PathBuf::from)
            .unwrap_or_else(|_| "data/calorie-man.db".into()),
        bind: std::env::var("CALORIE_MAN_BIND").unwrap_or_else(|_| "0.0.0.0:8484".into()),
    });
    match cmd {
        Cmd::Serve { db, bind } => serve(db, bind).await,
        Cmd::McpStdio { url } => mcp_stdio(url).await,
        Cmd::SeedDemo { db, days, force } => {
            if force {
                for suffix in ["", "-wal", "-shm"] {
                    let p = PathBuf::from(format!("{}{suffix}", db.display()));
                    if p.exists() {
                        std::fs::remove_file(&p)?;
                    }
                }
            }
            let state = AppState::new(Config::new(db.clone()))?;
            let mut c = state.pool.get()?;
            let n: i64 = c.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
            if n > 0 {
                anyhow::bail!(
                    "{} already has users; pass --force to delete it and start over",
                    db.display()
                );
            }
            calorie_man::seed::seed_demo(
                &mut c,
                days.clamp(1, 1000),
                chrono::Local::now().date_naive(),
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("Seeded {} with {days} days of demo data", db.display());
            Ok(())
        }
    }
}

async fn serve(db: PathBuf, bind: String) -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "calorie_man=info,tower_http=warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let state = AppState::new(Config::new(db.clone()))?;
    let app = calorie_man::app(state);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    let addr = listener.local_addr()?;
    tracing::info!("database: {}", db.display());
    tracing::info!("web UI:   http://{addr}/");
    tracing::info!("MCP:      http://{addr}/mcp  (add ?user=<name> to set a default user)");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
}

/// Newline-delimited JSON-RPC on stdin/stdout <-> POST /mcp.
async fn mcp_stdio(url: String) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut out = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let id = serde_json::from_str::<serde_json::Value>(&line)
            .ok()
            .and_then(|v| v.get("id").cloned());
        let resp = client
            .post(&url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(line)
            .send()
            .await;
        let body = match resp {
            Ok(r) if r.status().as_u16() == 202 => continue,
            Ok(r) => r.text().await.unwrap_or_default(),
            Err(e) => match id {
                Some(id) if !id.is_null() => serde_json::json!({"jsonrpc": "2.0", "id": id,
                    "error": {"code": -32000, "message": format!("calorie-man server unreachable at {url}: {e}")}})
                .to_string(),
                _ => {
                    eprintln!("calorie-man server unreachable at {url}: {e}");
                    continue;
                }
            },
        };
        let body = body.replace('\n', " ");
        out.write_all(body.trim().as_bytes()).await?;
        out.write_all(b"\n").await?;
        out.flush().await?;
    }
    Ok(())
}
