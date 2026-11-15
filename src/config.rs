use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub db_path: PathBuf,
    pub usda_base: String,
    pub off_base: String,
    pub off_search_base: String,
    /// USDA key from the environment; a key saved in Settings takes precedence.
    pub usda_api_key: Option<String>,
    /// Extra Origins allowed to call /mcp from a browser (the server's own origin is always allowed).
    pub allowed_origins: Vec<String>,
    pub user_agent: String,
    /// Disable all outbound requests (tests, air-gapped setups).
    pub offline: bool,
    /// Open Food Facts text search. Off in the browser demo, whose page origin the OFF search server
    /// does not allow; barcode lookups use a different OFF host that does.
    pub off_text_search: bool,
}

impl Config {
    pub fn new(db_path: PathBuf) -> Self {
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        Config {
            db_path,
            usda_base: env("CALORIE_MAN_USDA_BASE")
                .unwrap_or_else(|| "https://api.nal.usda.gov/fdc/v1".into()),
            off_base: env("CALORIE_MAN_OFF_BASE")
                .unwrap_or_else(|| "https://world.openfoodfacts.org".into()),
            off_search_base: env("CALORIE_MAN_OFF_SEARCH_BASE")
                .unwrap_or_else(|| "https://search.openfoodfacts.org".into()),
            usda_api_key: env("USDA_API_KEY"),
            allowed_origins: env("CALORIE_MAN_ALLOWED_ORIGINS")
                .map(|s| {
                    s.split(',')
                        .map(|x| x.trim().to_string())
                        .filter(|x| !x.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            user_agent: format!(
                "calorie-man/{} (self-hosted calorie tracker)",
                env!("CARGO_PKG_VERSION")
            ),
            offline: env("CALORIE_MAN_OFFLINE")
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false),
            off_text_search: true,
        }
    }
}
