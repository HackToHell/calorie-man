//! Online food databases: USDA FoodData Central and Open Food Facts. Both are normalized into
//! `Candidate`s, which can be imported into the shared food library.

use crate::db;
use crate::error::{bad, AppError, AppResult};
use crate::nutrients::{Nutrients, CATALOG};
use crate::service::foods::{self, normalize_barcode, Food, FoodInput, ServingInput};
use crate::AppState;
use rusqlite::OptionalExtension;
use serde::Serialize;
use serde_json::Value;

const CACHE_SECS: i64 = 7 * 24 * 3600;
/// Tries per request when the upstream answers with a transient error.
const UPSTREAM_ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, Serialize)]
pub struct CandServing {
    pub label: String,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    pub source: String,
    pub source_id: String,
    pub name: String,
    pub brand: Option<String>,
    pub barcode: Option<String>,
    pub category: Option<String>,
    pub data_type: Option<String>,
    pub basis_amount: f64,
    pub basis_unit: String,
    pub servings: Vec<CandServing>,
    pub nutrients: Nutrients,
    pub nutrient_count: usize,
    /// Id of the food in the local library if this item was already imported.
    pub in_library: Option<i64>,
    pub url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub results: Vec<Candidate>,
    /// Problems with individual providers (rate limits, timeouts); results from the others still count.
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    Usda,
    Off,
    All,
}

impl Source {
    pub fn parse(s: Option<&str>) -> AppResult<Source> {
        match s.map(|s| s.trim().to_lowercase()).as_deref() {
            None | Some("") | Some("all") | Some("any") => Ok(Source::All),
            Some("usda") | Some("fdc") => Ok(Source::Usda),
            Some("off") | Some("openfoodfacts") | Some("open_food_facts") => Ok(Source::Off),
            Some(o) => Err(bad(format!("source must be usda, off or all, got '{o}'"))),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// HTTP with a small SQLite cache

async fn usda_key(state: &AppState) -> String {
    let k = state
        .db(|c| db::get_setting(c, "usda_api_key"))
        .await
        .ok()
        .flatten();
    k.or(state.cfg.usda_api_key.clone())
        .unwrap_or_else(|| "DEMO_KEY".into())
}

async fn get_json(
    state: &AppState,
    url: &str,
    query: &[(&str, String)],
    secret: Option<(&str, String)>,
    provider: &str,
) -> AppResult<Value> {
    if state.cfg.offline {
        return Err(AppError::Upstream(format!(
            "{provider}: online lookups are disabled (CALORIE_MAN_OFFLINE)"
        )));
    }
    let cache_key = {
        let qs: Vec<String> = query.iter().map(|(k, v)| format!("{k}={v}")).collect();
        format!("{url}?{}", qs.join("&"))
    };
    let ck = cache_key.clone();
    let cached: Option<String> = state
        .db(move |c| {
            let now = chrono::Utc::now().timestamp();
            Ok(c.query_row(
                "SELECT body FROM external_cache WHERE url = ? AND fetched_at > ?",
                rusqlite::params![ck, now - CACHE_SECS],
                |r| r.get(0),
            )
            .optional()?)
        })
        .await?;
    if let Some(body) = cached {
        if let Ok(v) = serde_json::from_str(&body) {
            return Ok(v);
        }
    }
    let mut q: Vec<(&str, String)> = query.to_vec();
    if let Some((k, v)) = secret {
        q.push((k, v));
    }
    // USDA's front end intermittently answers valid requests with an HTML 400 (and gateways with
    // 502-504), so retry those a couple of times before giving up.
    let mut attempt = 0;
    let resp = loop {
        attempt += 1;
        let resp = state.http.get(url).query(&q).send().await.map_err(|e| {
            AppError::Upstream(format!("{provider} is unreachable: {}", e.without_url()))
        })?;
        let transient = matches!(resp.status().as_u16(), 400 | 502 | 503 | 504);
        if !transient || attempt >= UPSTREAM_ATTEMPTS {
            break resp;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200 * attempt as u64)).await;
    };
    let status = resp.status();
    if status.as_u16() == 404 {
        return Err(AppError::NotFound(format!("{provider}: not found")));
    }
    if status.as_u16() == 429 || (provider == "USDA" && status.as_u16() == 403) {
        return Err(AppError::Upstream(if provider == "USDA" {
            "USDA rate limit reached or key rejected. The shared DEMO_KEY allows about 30 requests per hour; \
             get a free key at https://fdc.nal.usda.gov/api-key-signup and add it in Settings."
                .into()
        } else {
            format!("{provider} rate limit reached; try again in a minute")
        }));
    }
    if !status.is_success() {
        return Err(AppError::Upstream(format!(
            "{provider} returned HTTP {status}"
        )));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| AppError::Upstream(format!("{provider}: {}", e.without_url())))?;
    let v: Value = serde_json::from_str(&text).map_err(|_| {
        AppError::Upstream(format!(
            "{provider} returned something that is not JSON (service may be down)"
        ))
    })?;
    state
        .db(move |c| {
            c.execute(
                "INSERT INTO external_cache (url, body, fetched_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(url) DO UPDATE SET body = excluded.body, fetched_at = excluded.fetched_at",
                rusqlite::params![cache_key, text, chrono::Utc::now().timestamp()],
            )?;
            Ok(())
        })
        .await?;
    Ok(v)
}

// ---------------------------------------------------------------------------------------------------------
// Text helpers

fn title_case_if_shouting(s: &str) -> String {
    let s = s.trim();
    let letters: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() || letters.iter().any(|c| c.is_lowercase()) {
        return s.to_string();
    }
    s.split(' ')
        .map(|w| {
            let lower = w.to_lowercase();
            let mut ch = lower.chars();
            match ch.next() {
                Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn clean_serving_label(s: &str) -> String {
    let s = s.trim().to_lowercase();
    let s = s.strip_prefix("1 ").unwrap_or(&s).trim().to_string();
    let s = s
        .trim_matches(|c: char| c == ',' || c.is_whitespace())
        .to_string();
    // "591 ml" or "30 g" alone says nothing beyond the amount.
    let only_measure = s.split_whitespace().collect::<Vec<_>>().as_slice().len() == 2
        && s.split_whitespace()
            .next()
            .map(|x| x.parse::<f64>().is_ok())
            .unwrap_or(false)
        && crate::units::lookup(s.split_whitespace().nth(1).unwrap_or("")).is_some();
    if s.is_empty() || only_measure {
        "serving".into()
    } else {
        s.chars().take(60).collect()
    }
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
    .filter(|x: &f64| x.is_finite())
}

fn str_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Array(a) => a.iter().find_map(str_of),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn mass_g(unit: &str) -> Option<f64> {
    match unit.to_lowercase().as_str() {
        "g" => Some(1.0),
        "mg" => Some(1e-3),
        "ug" | "µg" | "mcg" | "μg" => Some(1e-6),
        _ => None,
    }
}

fn convert(v: f64, from: &str, to: &str) -> Option<f64> {
    let f = from.to_lowercase();
    if f == to.to_lowercase() || (f == "kcal" && to == "kcal") {
        return Some(v);
    }
    if to == "kcal" {
        return if f == "kj" { Some(v / 4.184) } else { None };
    }
    Some(v * mass_g(&f)? / mass_g(to)?)
}

// ---------------------------------------------------------------------------------------------------------
// USDA FoodData Central

/// Map USDA nutrient numbers ("208", "203", ...) to the catalog.
pub fn usda_nutrients(items: &[(String, String, f64)]) -> Nutrients {
    let mut n = Nutrients::empty();
    let find = |num: &str| {
        items
            .iter()
            .find(|(k, _, _)| k == num)
            .map(|(_, u, v)| (u.as_str(), *v))
    };
    for (i, nut) in CATALOG.iter().enumerate() {
        for num in nut.usda {
            if let Some((unit, v)) = find(num) {
                if let Some(x) = convert(v, unit, nut.unit) {
                    n.0[i] = Some(x);
                    break;
                }
            }
        }
    }
    if n.get("calories").is_none() {
        if let Some((u, v)) = find("268") {
            n.set("calories", convert(v, u, "kcal"));
        }
    }
    if n.get("vitamin_d").is_none() {
        if let Some(("IU", v)) | Some(("iu", v)) = find("324") {
            n.set("vitamin_d", Some(v / 40.0));
        }
    }
    n.rounded()
}

fn usda_basis(v: &Value) -> (f64, String) {
    let unit = v["servingSizeUnit"].as_str().unwrap_or("").to_lowercase();
    if v["dataType"].as_str() == Some("Branded") && matches!(unit.as_str(), "ml" | "mlt" | "mll") {
        (100.0, "ml".into())
    } else {
        (100.0, "g".into())
    }
}

fn usda_name_brand(v: &Value) -> (String, Option<String>) {
    let mut name = title_case_if_shouting(v["description"].as_str().unwrap_or("Unnamed food"));
    let brand = v["brandName"]
        .as_str()
        .or(v["brandOwner"].as_str())
        .filter(|s| !s.trim().is_empty())
        .map(title_case_if_shouting);
    if let Some(b) = &brand {
        // "KIRKLAND SIGNATURE, STRAWBERRY SPREAD" -> "Strawberry Spread"
        let prefix = format!("{}, ", b.to_lowercase());
        if name.to_lowercase().starts_with(&prefix) {
            name = name[prefix.len()..].to_string();
        }
    }
    let mut brand = brand;
    if let Some(sub) = v["subbrandName"].as_str().filter(|s| !s.trim().is_empty()) {
        let sub = title_case_if_shouting(sub);
        // USDA splits "KIRKLAND SIGNATURE" into brand "KIRKLAND" + sub-brand "SIGNATURE".
        if sub.eq_ignore_ascii_case("signature")
            && brand
                .as_deref()
                .map(|b| !b.to_lowercase().ends_with("signature"))
                .unwrap_or(false)
        {
            brand = brand.map(|b| format!("{b} Signature"));
        } else if !name.to_lowercase().contains(&sub.to_lowercase()) {
            name = format!("{sub} {name}");
        }
    }
    (name, brand)
}

fn usda_servings(v: &Value, basis_unit: &str) -> Vec<CandServing> {
    let mut out: Vec<CandServing> = vec![];
    if let Some(size) = num(&v["servingSize"]).filter(|x| *x > 0.0) {
        let unit = v["servingSizeUnit"].as_str().unwrap_or("g").to_lowercase();
        let unit_ok = matches!(
            (basis_unit, unit.as_str()),
            ("g", "g" | "grm") | ("ml", "ml" | "mlt" | "mll")
        );
        if unit_ok {
            let label = v["householdServingFullText"]
                .as_str()
                .map(clean_serving_label)
                .unwrap_or_else(|| "serving".into());
            out.push(CandServing {
                label,
                amount: size,
            });
        }
    }
    // Detail endpoint: foodPortions. Search endpoint: foodMeasures.
    if let Some(portions) = v["foodPortions"].as_array() {
        // USDA's sequenceNumber is the intended order (the first one becomes the default serving).
        let mut portions: Vec<&Value> = portions.iter().collect();
        portions.sort_by_key(|p| p["sequenceNumber"].as_i64().unwrap_or(i64::MAX));
        for p in portions {
            let Some(g) = num(&p["gramWeight"]).filter(|x| *x > 0.0) else {
                continue;
            };
            let desc = p["portionDescription"]
                .as_str()
                .filter(|s| !s.is_empty() && *s != "Quantity not specified");
            let label = match desc {
                Some(d) => d.to_string(),
                None => {
                    let amount = num(&p["amount"]).unwrap_or(1.0);
                    let unit = p["measureUnit"]["name"]
                        .as_str()
                        .filter(|u| *u != "undetermined")
                        .unwrap_or("");
                    // Survey (FNDDS) modifiers are numeric portion codes ("90000"), not words.
                    let modifier = p["modifier"]
                        .as_str()
                        .filter(|m| !m.chars().all(|c| c.is_ascii_digit()))
                        .unwrap_or("");
                    let base = [unit, modifier]
                        .iter()
                        .filter(|s| !s.is_empty())
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ");
                    if base.is_empty() {
                        continue;
                    }
                    if amount == 1.0 {
                        base
                    } else {
                        format!("{} {base}", foods::fmt_num(amount))
                    }
                }
            };
            push_serving(&mut out, clean_serving_label(&label), g);
        }
    } else if let Some(measures) = v["foodMeasures"].as_array() {
        for m in measures {
            let Some(g) = num(&m["gramWeight"]).filter(|x| *x > 0.0) else {
                continue;
            };
            let Some(t) = m["disseminationText"]
                .as_str()
                .filter(|s| !s.is_empty() && *s != "Quantity not specified")
            else {
                continue;
            };
            push_serving(&mut out, clean_serving_label(t), g);
        }
    }
    out.truncate(12);
    out
}

fn push_serving(out: &mut Vec<CandServing>, label: String, amount: f64) {
    if !out.iter().any(|s| s.label.eq_ignore_ascii_case(&label)) {
        out.push(CandServing { label, amount });
    }
}

/// Parse a USDA food from either the search endpoint (`foodNutrients[].nutrientNumber/value`) or the
/// detail endpoint (`foodNutrients[].nutrient.number/amount`).
pub fn parse_usda(v: &Value) -> Option<Candidate> {
    let id = v["fdcId"].as_i64()?;
    let items: Vec<(String, String, f64)> = v["foodNutrients"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|n| {
                    let number = n["nutrientNumber"]
                        .as_str()
                        .or(n["nutrient"]["number"].as_str())?;
                    let unit = n["unitName"]
                        .as_str()
                        .or(n["nutrient"]["unitName"].as_str())?;
                    let val = num(&n["value"]).or_else(|| num(&n["amount"]))?;
                    Some((number.to_string(), unit.to_string(), val))
                })
                .collect()
        })
        .unwrap_or_default();
    let (basis_amount, basis_unit) = usda_basis(v);
    let (name, brand) = usda_name_brand(v);
    let nutrients = usda_nutrients(&items);
    Some(Candidate {
        source: "usda".into(),
        source_id: id.to_string(),
        name,
        brand,
        barcode: v["gtinUpc"]
            .as_str()
            .map(normalize_barcode)
            .filter(|b| !b.is_empty()),
        category: v["foodCategory"]
            .as_str()
            .or(v["brandedFoodCategory"].as_str())
            .or(v["foodCategory"]["description"].as_str())
            .map(String::from),
        data_type: v["dataType"].as_str().map(String::from),
        servings: usda_servings(v, &basis_unit),
        basis_amount,
        basis_unit,
        nutrient_count: nutrients.known_count(),
        nutrients,
        in_library: None,
        url: Some(format!(
            "https://fdc.nal.usda.gov/food-details/{id}/nutrients"
        )),
    })
}

/// "costco milk" -> "+kirkland +milk": every word required, store brands mapped to label brands.
pub fn usda_query(q: &str) -> String {
    let toks = foods::tokens(q);
    if toks.iter().all(|t| t.chars().all(|c| c.is_ascii_digit())) && !toks.is_empty() {
        return toks.join(" ");
    }
    toks.iter()
        .map(|t| {
            if t == "costco" {
                "kirkland".to_string()
            } else {
                t.clone()
            }
        })
        .map(|t| format!("+{}", t.replace('\'', "")))
        .collect::<Vec<_>>()
        .join(" ")
}

pub async fn usda_search(
    state: &AppState,
    q: &str,
    limit: usize,
    kind: &str,
) -> AppResult<Vec<Candidate>> {
    let types = match kind {
        "generic" => "Foundation,SR Legacy,Survey (FNDDS)",
        "branded" => "Branded",
        _ => "Foundation,SR Legacy,Survey (FNDDS),Branded",
    };
    let url = format!("{}/foods/search", state.cfg.usda_base);
    let query = [
        ("query", usda_query(q)),
        ("pageSize", limit.clamp(1, 50).to_string()),
        ("dataType", types.to_string()),
    ];
    let v = get_json(
        state,
        &url,
        &query,
        Some(("api_key", usda_key(state).await)),
        "USDA",
    )
    .await?;
    Ok(v["foods"]
        .as_array()
        .map(|a| a.iter().filter_map(parse_usda).collect())
        .unwrap_or_default())
}

pub async fn usda_food(state: &AppState, id: &str) -> AppResult<Candidate> {
    if !id.chars().all(|c| c.is_ascii_digit()) || id.is_empty() {
        return Err(bad("USDA ids are numbers (fdcId)"));
    }
    let url = format!("{}/food/{id}", state.cfg.usda_base);
    let v = get_json(
        state,
        &url,
        &[],
        Some(("api_key", usda_key(state).await)),
        "USDA",
    )
    .await?;
    parse_usda(&v).ok_or_else(|| AppError::Upstream("USDA returned an unexpected response".into()))
}

async fn usda_barcode(state: &AppState, code: &str) -> AppResult<Option<Candidate>> {
    let hits = usda_search(state, code, 10, "branded").await?;
    let want = code.trim_start_matches('0');
    Ok(hits.into_iter().find(|c| {
        c.barcode
            .as_deref()
            .map(|b| b.trim_start_matches('0') == want)
            .unwrap_or(false)
    }))
}

// ---------------------------------------------------------------------------------------------------------
// Open Food Facts

const OFF_FIELDS: &str = "code,product_name,product_name_en,generic_name,generic_name_en,abbreviated_product_name,brands,\
quantity,serving_size,serving_quantity,serving_quantity_unit,nutrition_data_per,nutriments,product_quantity_unit,categories";

fn norm_words(s: &str) -> String {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Open Food Facts often stores only the brand as the product name ("Kirkland"). Prefer a name that says
/// what the product is; fall back to "<brand> <most specific category>". Returns (name, informative).
fn off_name(p: &Value, brand: Option<&str>) -> Option<(String, bool)> {
    let brand_n = brand.map(norm_words).unwrap_or_default();
    let names: Vec<String> = [
        "product_name",
        "product_name_en",
        "generic_name_en",
        "generic_name",
        "abbreviated_product_name",
    ]
    .iter()
    .filter_map(|k| str_of(&p[*k]))
    .collect();
    let informative = |n: &str| {
        let w = norm_words(n);
        !w.is_empty() && w != brand_n && !brand_n.split(' ').any(|b| b == w)
    };
    if let Some(n) = names.iter().find(|n| informative(n)) {
        return Some((title_case_if_shouting(n), true));
    }
    let category = str_of(&p["categories"]).and_then(|c| {
        c.split(',')
            .map(str::trim)
            .rfind(|x| !x.is_empty() && !x.contains(':'))
            .map(String::from)
    });
    match (category, names.first()) {
        (Some(cat), _) => Some((
            format!("{} ({})", cat, brand.unwrap_or("unknown brand")),
            true,
        )),
        (None, Some(n)) => Some((title_case_if_shouting(n), false)),
        (None, None) => None,
    }
}

pub fn parse_off(p: &Value) -> Option<Candidate> {
    let code = str_of(&p["code"])
        .map(|c| normalize_barcode(&c))
        .filter(|c| !c.is_empty())?;
    let brand = str_of(&p["brands"])
        .map(|b| b.split(',').next().unwrap_or("").trim().to_string())
        .filter(|b| !b.is_empty());
    let (name, informative) = off_name(p, brand.as_deref())?;
    let brand = brand.map(|b| title_case_if_shouting(&b));
    let nm = &p["nutriments"];
    let per = p["nutrition_data_per"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();
    let sq_unit = p["serving_quantity_unit"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();
    let pq_unit = p["product_quantity_unit"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();
    let liquid = per.contains("ml") || sq_unit == "ml" || pq_unit == "ml";
    let read = |suffix: &str| -> Nutrients {
        let mut n = Nutrients::empty();
        for (i, nut) in CATALOG.iter().enumerate() {
            if let Some((key, factor)) = nut.off {
                if let Some(v) = num(&nm[format!("{key}_{suffix}")]) {
                    if v >= 0.0 {
                        n.0[i] = Some(v * factor);
                    }
                }
            }
        }
        if n.get("calories").is_none() {
            let kj = num(&nm[format!("energy-kj_{suffix}")])
                .or_else(|| num(&nm[format!("energy_{suffix}")]));
            n.set("calories", kj.map(|k| k / 4.184));
        }
        n.rounded()
    };
    let per100 = read("100g");
    let (basis_amount, basis_unit, nutrients) = if !per100.is_empty() {
        (100.0, if liquid { "ml" } else { "g" }.to_string(), per100)
    } else {
        let ps = read("serving");
        if ps.is_empty() {
            return None;
        }
        (1.0, "serving".to_string(), ps)
    };
    let mut servings = vec![];
    if basis_unit != "serving" {
        if let Some(sq) = num(&p["serving_quantity"]).filter(|x| *x > 0.0 && *x < 10000.0) {
            let unit = if sq_unit.is_empty() {
                "g".to_string()
            } else {
                sq_unit.clone()
            };
            if unit == basis_unit {
                let label = p["serving_size"]
                    .as_str()
                    .map(clean_serving_label)
                    .unwrap_or_else(|| "serving".into());
                servings.push(CandServing { label, amount: sq });
            }
        }
    }
    Some(Candidate {
        source: "off".into(),
        source_id: code.clone(),
        name: name.trim().to_string(),
        brand,
        barcode: Some(code.clone()),
        category: str_of(&p["categories"])
            .map(|c| c.split(',').next_back().unwrap_or("").trim().to_string()),
        data_type: Some(
            if informative {
                "Open Food Facts"
            } else {
                "Open Food Facts (unnamed)"
            }
            .into(),
        ),
        basis_amount,
        basis_unit,
        servings,
        nutrient_count: nutrients.known_count(),
        nutrients,
        in_library: None,
        url: Some(format!("https://world.openfoodfacts.org/product/{code}")),
    })
}

pub async fn off_search(state: &AppState, q: &str, limit: usize) -> AppResult<Vec<Candidate>> {
    let q = foods::tokens(q)
        .into_iter()
        .map(|t| if t == "costco" { "kirkland".into() } else { t })
        .collect::<Vec<_>>()
        .join(" ");
    let url = format!("{}/search", state.cfg.off_search_base);
    // Ask for extra hits because unnamed products are dropped below.
    let query = [
        ("q", q),
        ("page_size", (limit * 2).clamp(1, 50).to_string()),
        ("fields", OFF_FIELDS.to_string()),
    ];
    let v = get_json(state, &url, &query, None, "Open Food Facts").await?;
    let words: Vec<String> = query[0].1.split(' ').map(|w| w.to_string()).collect();
    let mut list: Vec<Candidate> = v["hits"]
        .as_array()
        .or(v["products"].as_array())
        .map(|a| {
            a.iter()
                .filter_map(parse_off)
                .filter(|c| c.nutrients.get("calories").is_some())
                .collect()
        })
        .unwrap_or_default();
    // Drop products whose name is only the brand (nobody can tell what they are), then rank by how many
    // query words describe the product itself rather than the brand.
    list.retain(|c| c.data_type.as_deref() != Some("Open Food Facts (unnamed)"));
    list.sort_by(|a, b| {
        off_score(b, &words)
            .partial_cmp(&off_score(a, &words))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    list.truncate(limit);
    Ok(list)
}

fn off_score(c: &Candidate, words: &[String]) -> f64 {
    let name = norm_words(&c.name);
    let brand = c.brand.as_deref().map(norm_words).unwrap_or_default();
    let mut s = 0.0;
    for w in words {
        if name.split(' ').any(|n| n == w) && !brand.split(' ').any(|b| b == w) {
            s += 3.0;
        } else if name.contains(w.as_str()) || brand.contains(w.as_str()) {
            s += 1.0;
        }
    }
    s + c.nutrient_count as f64 * 0.05 + if c.servings.is_empty() { 0.0 } else { 0.5 }
}

pub async fn off_product(state: &AppState, code: &str) -> AppResult<Option<Candidate>> {
    let code = normalize_barcode(code);
    if code.is_empty() {
        return Err(bad("barcode must contain digits"));
    }
    let url = format!("{}/api/v2/product/{code}.json", state.cfg.off_base);
    let v = match get_json(
        state,
        &url,
        &[("fields", OFF_FIELDS.to_string())],
        None,
        "Open Food Facts",
    )
    .await
    {
        Ok(v) => v,
        Err(AppError::NotFound(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    if v["status"].as_i64() != Some(1) {
        return Ok(None);
    }
    let mut p = v["product"].clone();
    if p["code"].is_null() {
        p["code"] = Value::String(code);
    }
    Ok(parse_off(&p))
}

// ---------------------------------------------------------------------------------------------------------
// Public API

async fn mark_in_library(state: &AppState, mut list: Vec<Candidate>) -> AppResult<Vec<Candidate>> {
    let keys: Vec<(String, String, Option<String>)> = list
        .iter()
        .map(|c| (c.source.clone(), c.source_id.clone(), c.barcode.clone()))
        .collect();
    let ids = state
        .db(move |c| {
            let mut out = vec![];
            for (s, id, bc) in keys {
                let f = match foods::find_by_source(c, &s, &id)? {
                    Some(f) => Some(f.id),
                    None => match bc {
                        Some(b) => foods::find_by_barcode(c, &b)?.map(|f| f.id),
                        None => None,
                    },
                };
                out.push(f);
            }
            Ok(out)
        })
        .await?;
    for (c, id) in list.iter_mut().zip(ids) {
        c.in_library = id;
    }
    Ok(list)
}

pub async fn search(
    state: &AppState,
    q: &str,
    source: Source,
    limit: usize,
    usda_kind: &str,
) -> AppResult<SearchResult> {
    let q = q.trim();
    if q.is_empty() {
        return Err(bad("search text is required"));
    }
    let limit = limit.clamp(1, 50);
    let (u, o) = tokio::join!(
        async {
            if source != Source::Off {
                Some(usda_search(state, q, limit, usda_kind).await)
            } else {
                None
            }
        },
        async {
            if source != Source::Usda {
                Some(off_search(state, q, limit).await)
            } else {
                None
            }
        }
    );
    let mut results = vec![];
    let mut errors = vec![];
    for r in [u, o].into_iter().flatten() {
        match r {
            Ok(mut v) => results.append(&mut v),
            Err(e) => errors.push(e.to_string()),
        }
    }
    if results.is_empty()
        && !errors.is_empty()
        && errors.len()
            == [source != Source::Off, source != Source::Usda]
                .iter()
                .filter(|x| **x)
                .count()
    {
        return Err(AppError::Upstream(errors.join("; ")));
    }
    // Interleave so both sources show up near the top.
    let (mut us, mut os): (Vec<_>, Vec<_>) = results.into_iter().partition(|c| c.source == "usda");
    us.reverse();
    os.reverse();
    let mut merged = vec![];
    while !us.is_empty() || !os.is_empty() {
        if let Some(x) = us.pop() {
            merged.push(x);
        }
        if let Some(x) = os.pop() {
            merged.push(x);
        }
    }
    Ok(SearchResult {
        results: mark_in_library(state, merged).await?,
        errors,
    })
}

pub async fn fetch(state: &AppState, source: &str, id: &str) -> AppResult<Candidate> {
    match Source::parse(Some(source))? {
        Source::Usda => usda_food(state, id.trim()).await,
        Source::Off => off_product(state, id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("no Open Food Facts product {id}"))),
        Source::All => Err(bad("source must be 'usda' or 'off'")),
    }
}

#[derive(Debug, Serialize)]
pub struct BarcodeResult {
    pub barcode: String,
    /// Already in the shared library.
    pub food: Option<Food>,
    /// Found online (import it to use it).
    pub candidate: Option<Candidate>,
    pub errors: Vec<String>,
}

/// Look a barcode up locally, then on Open Food Facts, then in USDA branded foods.
pub async fn barcode(state: &AppState, code: &str) -> AppResult<BarcodeResult> {
    let code = normalize_barcode(code);
    if code.len() < 6 || code.len() > 14 {
        return Err(bad("barcode must have 6-14 digits"));
    }
    let c2 = code.clone();
    let local = state.db(move |c| foods::find_by_barcode(c, &c2)).await?;
    if local.is_some() {
        return Ok(BarcodeResult {
            barcode: code,
            food: local,
            candidate: None,
            errors: vec![],
        });
    }
    let mut errors = vec![];
    let mut cand = match off_product(state, &code).await {
        Ok(c) => c,
        Err(e) => {
            errors.push(e.to_string());
            None
        }
    };
    if cand.is_none() {
        match usda_barcode(state, &code).await {
            Ok(c) => cand = c,
            Err(e) => errors.push(e.to_string()),
        }
    }
    let cand = match cand {
        Some(c) => mark_in_library(state, vec![c]).await?.pop(),
        None => None,
    };
    Ok(BarcodeResult {
        barcode: code,
        food: None,
        candidate: cand,
        errors,
    })
}

pub fn candidate_to_input(
    c: &Candidate,
    aliases: Option<String>,
    name: Option<String>,
) -> FoodInput {
    FoodInput {
        name: name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| c.name.clone()),
        brand: c.brand.clone(),
        kind: Some("food".into()),
        basis_amount: Some(c.basis_amount),
        basis_unit: Some(c.basis_unit.clone()),
        barcode: c.barcode.clone(),
        aliases,
        notes: c
            .category
            .clone()
            .map(|cat| format!("{} · {cat}", c.data_type.clone().unwrap_or_default())),
        nutrients: c.nutrients.clone(),
        servings: Some(
            c.servings
                .iter()
                .enumerate()
                .map(|(i, s)| ServingInput {
                    id: None,
                    label: s.label.clone(),
                    amount: s.amount,
                    unit: None,
                    is_default: i == 0,
                })
                .collect(),
        ),
        source: Some(c.source.clone()),
        source_id: Some(c.source_id.clone()),
        ingredients: None,
        recipe_yield: None,
        cooked_weight_g: None,
    }
}

#[derive(Debug, Serialize)]
pub struct Imported {
    pub food: Food,
    /// false when the item was already in the library.
    pub created: bool,
    pub warnings: Vec<String>,
}

/// Import an online item into the shared library (or return the existing copy).
pub async fn import(
    state: &AppState,
    source: &str,
    id: &str,
    created_by: Option<i64>,
    aliases: Option<String>,
    name: Option<String>,
) -> AppResult<Imported> {
    let src = match Source::parse(Some(source))? {
        Source::Usda => "usda",
        Source::Off => "off",
        Source::All => return Err(bad("source must be 'usda' or 'off'")),
    };
    let id_norm = if src == "off" {
        normalize_barcode(id)
    } else {
        id.trim().to_string()
    };
    let (s2, i2) = (src.to_string(), id_norm.clone());
    if let Some(f) = state
        .db(move |c| foods::find_by_source(c, &s2, &i2))
        .await?
    {
        return Ok(Imported {
            food: f,
            created: false,
            warnings: vec![],
        });
    }
    let cand = fetch(state, src, &id_norm).await?;
    state
        .db(move |c| {
            if let Some(b) = &cand.barcode {
                if let Some(f) = foods::find_by_barcode(c, b)? {
                    return Ok(Imported {
                        food: f,
                        created: false,
                        warnings: vec![],
                    });
                }
            }
            let mut inp = candidate_to_input(&cand, aliases, name);
            let mut warnings = vec![];
            if cand.nutrients.get("calories").is_none() {
                warnings.push("the source has no calorie value for this item".into());
            }
            // Source data can be messy; do not let a sanity check block an import of a real product.
            let saved = match foods::create(c, inp.clone(), created_by) {
                Ok(s) => s,
                Err(AppError::BadRequest(msg)) if msg.contains("impossible") => {
                    warnings.push(format!(
                        "source data looked wrong ({msg}); macros were dropped, please review"
                    ));
                    for k in ["protein", "carbs", "fat", "calories"] {
                        inp.nutrients.set(k, None);
                    }
                    foods::create(c, inp, created_by)?
                }
                Err(e) => return Err(e),
            };
            warnings.extend(saved.warnings);
            Ok(Imported {
                food: saved.food,
                created: true,
                warnings,
            })
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Value {
        let p = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    }

    #[test]
    fn parses_usda_branded_detail() {
        let c = parse_usda(&fixture("usda_food_2460407.json")).unwrap();
        assert_eq!(c.name, "Organic Whole Milk");
        assert_eq!(c.brand.as_deref(), Some("Kirkland Signature"));
        assert_eq!(c.basis_unit, "ml");
        assert_eq!(c.nutrients.get("calories"), Some(62.0));
        assert_eq!(c.nutrients.get("protein"), Some(3.33));
        assert_eq!(c.nutrients.get("vitamin_d"), Some(1.25), "50 IU / 40");
        assert_eq!(
            c.nutrients.get("vitamin_a"),
            None,
            "IU vitamin A is ambiguous and skipped"
        );
        assert_eq!(c.servings.len(), 1);
        assert_eq!(c.servings[0].label, "cup");
        assert_eq!(c.servings[0].amount, 240.0);
        assert_eq!(c.barcode.as_deref(), Some("096619163014"));
    }

    #[test]
    fn parses_usda_sr_legacy_detail_with_portions() {
        let c = parse_usda(&fixture("usda_food_173944.json")).unwrap();
        assert_eq!(c.name, "Bananas, raw");
        assert_eq!(c.basis_unit, "g");
        assert_eq!(c.nutrients.get("calories"), Some(89.0));
        assert_eq!(c.nutrients.get("potassium"), Some(358.0));
        assert!(
            c.nutrient_count > 25,
            "generic foods have full vitamin profiles: {}",
            c.nutrient_count
        );
        assert!(
            c.servings.iter().any(|s| s.label.contains("large")),
            "{:?}",
            c.servings
        );
    }

    #[test]
    fn fndds_portion_codes_are_not_serving_labels() {
        // Survey (FNDDS) portions carry a numeric food-code `modifier` ("90000" = quantity not
        // specified); those must never become labels, and USDA's sequenceNumber sets the order.
        let c = parse_usda(&fixture("usda_food_2709223.json")).unwrap();
        assert_eq!(c.name, "Avocado, raw");
        let labels: Vec<&str> = c.servings.iter().map(|s| s.label.as_str()).collect();
        assert!(
            labels
                .iter()
                .all(|l| !l.chars().all(|ch| ch.is_ascii_digit())),
            "{labels:?}"
        );
        assert_eq!(
            labels,
            ["slice", "fruit", "cup", "cup, mashed or pureed"],
            "in USDA sequence order"
        );
        assert_eq!(c.servings[1].amount, 150.0);
    }

    #[test]
    fn parses_usda_search_results() {
        let v = fixture("usda_search_kirkland_milk.json");
        let list: Vec<Candidate> = v["foods"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(parse_usda)
            .collect();
        assert!(list.len() >= 5);
        let milk = list.iter().find(|c| c.source_id == "2460407").unwrap();
        assert_eq!(milk.nutrients.get("calories"), Some(62.0));
        assert_eq!(milk.servings[0].amount, 240.0);
    }

    #[test]
    fn parses_off_products() {
        let v = fixture("off_product_nutella.json");
        let c = parse_off(&v["product"]).unwrap();
        assert_eq!(c.name, "Nutella");
        assert_eq!(c.brand.as_deref(), Some("Ferrero"));
        assert_eq!(c.basis_unit, "g");
        assert_eq!(c.nutrients.get("calories"), Some(539.0));
        assert_eq!(
            c.nutrients.get("sodium"),
            Some(43.0),
            "OFF stores grams; we store mg"
        );

        let v = fixture("off_product_kirkland.json");
        let c = parse_off(&v["product"]).unwrap();
        assert_eq!(c.basis_unit, "ml");
        assert_eq!(c.servings[0].amount, 591.0);
        assert!(c.servings[0].label.starts_with("bottle"));
        assert_eq!(c.nutrients.get("vitamin_c"), Some(15.2284));
    }

    #[test]
    fn off_names_that_are_only_the_brand_use_the_category() {
        let p = serde_json::json!({"code": "0096619204205", "product_name": "Kirkland", "brands": ["Kirkland Signature"],
            "categories": "Beverages,Waters,Vitamin water", "nutriments": {"energy-kcal_100g": 0}});
        let c = parse_off(&p).unwrap();
        assert_eq!(c.name, "Vitamin water (Kirkland Signature)");
        let p = serde_json::json!({"code": "02831305", "product_name": "KIRKLAND", "brands": "Kirkland", "nutriments": {"energy-kcal_100g": 800}});
        assert_eq!(
            parse_off(&p).unwrap().data_type.as_deref(),
            Some("Open Food Facts (unnamed)")
        );
        let p = serde_json::json!({"code": "1", "product_name": "Kirkland", "product_name_en": "Organic Whole Milk", "brands": "Kirkland", "nutriments": {"energy-kcal_100g": 62}});
        assert_eq!(parse_off(&p).unwrap().name, "Organic Whole Milk");
    }

    #[test]
    fn usda_signature_subbrand_belongs_to_the_brand() {
        let v = serde_json::json!({"fdcId": 1, "description": "MILK CHOCOLATE ALMONDS", "brandName": "KIRKLAND",
            "subbrandName": "SIGNATURE", "dataType": "Branded", "foodNutrients": []});
        let c = parse_usda(&v).unwrap();
        assert_eq!(c.name, "Milk Chocolate Almonds");
        assert_eq!(c.brand.as_deref(), Some("Kirkland Signature"));
    }

    #[test]
    fn query_building() {
        assert_eq!(usda_query("Costco milk"), "+kirkland +milk");
        assert_eq!(usda_query("096619163014"), "096619163014");
        assert_eq!(clean_serving_label("1 Cup"), "cup");
        assert_eq!(clean_serving_label("591 ml"), "serving");
        assert_eq!(
            title_case_if_shouting("KIRKLAND SIGNATURE"),
            "Kirkland Signature"
        );
        assert_eq!(title_case_if_shouting("Bananas, raw"), "Bananas, raw");
    }
}
