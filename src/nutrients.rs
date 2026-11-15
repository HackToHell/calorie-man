//! The nutrient catalog. Every nutrient the app knows about is listed here once; the database columns,
//! API validation, MCP schema, external API mapping and UI forms are all derived from this table.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Energy,
    Macros,
    Fats,
    Minerals,
    Vitamins,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Nutrient {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
    pub group: Group,
    /// FDA adult daily value (2,000 kcal reference), if one exists.
    pub dv: Option<f64>,
    /// When true the daily value is an upper limit (sodium, saturated fat), not a target.
    pub limit: bool,
    #[serde(skip)]
    pub usda: &'static [&'static str],
    /// Open Food Facts nutriment key and the factor from OFF's base unit (g or kcal) to ours.
    #[serde(skip)]
    pub off: Option<(&'static str, f64)>,
    pub aliases: &'static [&'static str],
}

macro_rules! n {
    ($key:expr, $label:expr, $unit:expr, $group:ident, $dv:expr, $limit:expr, $usda:expr, $off:expr, $aliases:expr) => {
        Nutrient {
            key: $key,
            label: $label,
            unit: $unit,
            group: Group::$group,
            dv: $dv,
            limit: $limit,
            usda: $usda,
            off: $off,
            aliases: $aliases,
        }
    };
}

pub const CATALOG: &[Nutrient] = &[
    n!(
        "calories",
        "Calories",
        "kcal",
        Energy,
        None,
        false,
        &["208", "957", "958"],
        Some(("energy-kcal", 1.0)),
        &["kcal", "energy", "calorie", "cal", "energy_kcal", "cals"]
    ),
    n!(
        "protein",
        "Protein",
        "g",
        Macros,
        Some(50.0),
        false,
        &["203"],
        Some(("proteins", 1.0)),
        &["proteins", "protein_g"]
    ),
    n!(
        "carbs",
        "Carbohydrates",
        "g",
        Macros,
        Some(275.0),
        false,
        &["205", "205.2"],
        Some(("carbohydrates", 1.0)),
        &[
            "carbohydrates",
            "carbohydrate",
            "carb",
            "total_carbs",
            "total_carbohydrate",
            "carbs_g"
        ]
    ),
    n!(
        "fat",
        "Fat",
        "g",
        Macros,
        Some(78.0),
        false,
        &["204"],
        Some(("fat", 1.0)),
        &["total_fat", "fats", "lipid", "fat_g"]
    ),
    n!(
        "fiber",
        "Fiber",
        "g",
        Macros,
        Some(28.0),
        false,
        &["291"],
        Some(("fiber", 1.0)),
        &["fibre", "dietary_fiber", "fiber_g"]
    ),
    n!(
        "sugar",
        "Sugars",
        "g",
        Macros,
        None,
        false,
        &["269", "269.3"],
        Some(("sugars", 1.0)),
        &["sugars", "total_sugars", "total_sugar"]
    ),
    n!(
        "added_sugar",
        "Added sugars",
        "g",
        Macros,
        Some(50.0),
        true,
        &["539"],
        Some(("added-sugars", 1.0)),
        &["added_sugars", "addedsugar"]
    ),
    n!(
        "saturated_fat",
        "Saturated fat",
        "g",
        Fats,
        Some(20.0),
        true,
        &["606"],
        Some(("saturated-fat", 1.0)),
        &["sat_fat", "saturated", "saturated_fats"]
    ),
    n!(
        "trans_fat",
        "Trans fat",
        "g",
        Fats,
        None,
        true,
        &["605"],
        Some(("trans-fat", 1.0)),
        &["trans", "trans_fats"]
    ),
    n!(
        "monounsaturated_fat",
        "Monounsaturated fat",
        "g",
        Fats,
        None,
        false,
        &["645"],
        Some(("monounsaturated-fat", 1.0)),
        &["mufa", "mono_fat", "monounsaturated"]
    ),
    n!(
        "polyunsaturated_fat",
        "Polyunsaturated fat",
        "g",
        Fats,
        None,
        false,
        &["646"],
        Some(("polyunsaturated-fat", 1.0)),
        &["pufa", "poly_fat", "polyunsaturated"]
    ),
    n!(
        "cholesterol",
        "Cholesterol",
        "mg",
        Fats,
        Some(300.0),
        true,
        &["601"],
        Some(("cholesterol", 1000.0)),
        &["chol"]
    ),
    n!(
        "sodium",
        "Sodium",
        "mg",
        Minerals,
        Some(2300.0),
        true,
        &["307"],
        Some(("sodium", 1000.0)),
        &["na"]
    ),
    n!(
        "potassium",
        "Potassium",
        "mg",
        Minerals,
        Some(4700.0),
        false,
        &["306"],
        Some(("potassium", 1000.0)),
        &["k"]
    ),
    n!(
        "calcium",
        "Calcium",
        "mg",
        Minerals,
        Some(1300.0),
        false,
        &["301"],
        Some(("calcium", 1000.0)),
        &["ca"]
    ),
    n!(
        "iron",
        "Iron",
        "mg",
        Minerals,
        Some(18.0),
        false,
        &["303"],
        Some(("iron", 1000.0)),
        &["fe"]
    ),
    n!(
        "magnesium",
        "Magnesium",
        "mg",
        Minerals,
        Some(420.0),
        false,
        &["304"],
        Some(("magnesium", 1000.0)),
        &[]
    ),
    n!(
        "phosphorus",
        "Phosphorus",
        "mg",
        Minerals,
        Some(1250.0),
        false,
        &["305"],
        Some(("phosphorus", 1000.0)),
        &[]
    ),
    n!(
        "zinc",
        "Zinc",
        "mg",
        Minerals,
        Some(11.0),
        false,
        &["309"],
        Some(("zinc", 1000.0)),
        &["zn"]
    ),
    n!(
        "copper",
        "Copper",
        "mg",
        Minerals,
        Some(0.9),
        false,
        &["312"],
        Some(("copper", 1000.0)),
        &["cu"]
    ),
    n!(
        "manganese",
        "Manganese",
        "mg",
        Minerals,
        Some(2.3),
        false,
        &["315"],
        Some(("manganese", 1000.0)),
        &["mn"]
    ),
    n!(
        "selenium",
        "Selenium",
        "µg",
        Minerals,
        Some(55.0),
        false,
        &["317"],
        Some(("selenium", 1_000_000.0)),
        &["se"]
    ),
    n!(
        "iodine",
        "Iodine",
        "µg",
        Minerals,
        Some(150.0),
        false,
        &["314"],
        Some(("iodine", 1_000_000.0)),
        &[]
    ),
    n!(
        "vitamin_a",
        "Vitamin A (RAE)",
        "µg",
        Vitamins,
        Some(900.0),
        false,
        &["320"],
        Some(("vitamin-a", 1_000_000.0)),
        &["vit_a", "vitamina", "retinol"]
    ),
    n!(
        "vitamin_c",
        "Vitamin C",
        "mg",
        Vitamins,
        Some(90.0),
        false,
        &["401"],
        Some(("vitamin-c", 1000.0)),
        &["vit_c", "vitaminc", "ascorbic_acid"]
    ),
    n!(
        "vitamin_d",
        "Vitamin D",
        "µg",
        Vitamins,
        Some(20.0),
        false,
        &["328"],
        Some(("vitamin-d", 1_000_000.0)),
        &["vit_d", "vitamind"]
    ),
    n!(
        "vitamin_e",
        "Vitamin E",
        "mg",
        Vitamins,
        Some(15.0),
        false,
        &["323"],
        Some(("vitamin-e", 1000.0)),
        &["vit_e", "vitamine"]
    ),
    n!(
        "vitamin_k",
        "Vitamin K",
        "µg",
        Vitamins,
        Some(120.0),
        false,
        &["430"],
        Some(("vitamin-k", 1_000_000.0)),
        &["vit_k", "vitamink"]
    ),
    n!(
        "thiamin",
        "Thiamin (B1)",
        "mg",
        Vitamins,
        Some(1.2),
        false,
        &["404"],
        Some(("vitamin-b1", 1000.0)),
        &["vitamin_b1", "b1", "thiamine"]
    ),
    n!(
        "riboflavin",
        "Riboflavin (B2)",
        "mg",
        Vitamins,
        Some(1.3),
        false,
        &["405"],
        Some(("vitamin-b2", 1000.0)),
        &["vitamin_b2", "b2"]
    ),
    n!(
        "niacin",
        "Niacin (B3)",
        "mg",
        Vitamins,
        Some(16.0),
        false,
        &["406"],
        Some(("vitamin-pp", 1000.0)),
        &["vitamin_b3", "b3"]
    ),
    n!(
        "pantothenic_acid",
        "Pantothenic acid (B5)",
        "mg",
        Vitamins,
        Some(5.0),
        false,
        &["410"],
        Some(("pantothenic-acid", 1000.0)),
        &["vitamin_b5", "b5"]
    ),
    n!(
        "vitamin_b6",
        "Vitamin B6",
        "mg",
        Vitamins,
        Some(1.7),
        false,
        &["415"],
        Some(("vitamin-b6", 1000.0)),
        &["b6", "vit_b6"]
    ),
    n!(
        "biotin",
        "Biotin (B7)",
        "µg",
        Vitamins,
        Some(30.0),
        false,
        &["416"],
        Some(("biotin", 1_000_000.0)),
        &["vitamin_b7", "b7"]
    ),
    n!(
        "folate",
        "Folate (DFE)",
        "µg",
        Vitamins,
        Some(400.0),
        false,
        &["435", "417"],
        Some(("vitamin-b9", 1_000_000.0)),
        &["folic_acid", "vitamin_b9", "b9", "folate_dfe"]
    ),
    n!(
        "vitamin_b12",
        "Vitamin B12",
        "µg",
        Vitamins,
        Some(2.4),
        false,
        &["418"],
        Some(("vitamin-b12", 1_000_000.0)),
        &["b12", "vit_b12", "cobalamin"]
    ),
    n!(
        "choline",
        "Choline",
        "mg",
        Vitamins,
        Some(550.0),
        false,
        &["421"],
        None,
        &[]
    ),
    n!(
        "caffeine",
        "Caffeine",
        "mg",
        Other,
        None,
        false,
        &["262"],
        Some(("caffeine", 1000.0)),
        &[]
    ),
    n!(
        "alcohol",
        "Alcohol",
        "g",
        Other,
        None,
        false,
        &["221"],
        None,
        &["ethanol"]
    ),
];

pub const N: usize = CATALOG.len();

pub fn index_of(key: &str) -> Option<usize> {
    CATALOG.iter().position(|n| n.key == key)
}

pub fn idx(key: &str) -> usize {
    index_of(key).unwrap_or_else(|| panic!("unknown nutrient {key}"))
}

/// Resolve a key or an alias ("kcal", "Carbohydrates", "vitamin-c") to a catalog index.
pub fn resolve_key(raw: &str) -> Option<usize> {
    let k = raw.trim().to_lowercase().replace(['-', ' '], "_");
    let k = k
        .trim_end_matches("_g")
        .trim_end_matches("_mg")
        .trim_end_matches("_ug")
        .trim_end_matches("_mcg")
        .trim_end_matches("_kcal")
        .to_string();
    let candidates = [
        k.clone(),
        raw.trim().to_lowercase().replace(['-', ' '], "_"),
    ];
    for c in candidates.iter() {
        if let Some(i) = CATALOG
            .iter()
            .position(|n| n.key == c || n.aliases.contains(&c.as_str()))
        {
            return Some(i);
        }
    }
    None
}

pub fn column(i: usize) -> String {
    format!("n_{}", CATALOG[i].key)
}

/// `n_calories, n_protein, ...` optionally prefixed with a table alias.
pub fn columns(prefix: &str) -> String {
    CATALOG
        .iter()
        .map(|n| format!("{prefix}n_{}", n.key))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A full set of nutrient amounts. `None` means unknown, which is different from zero.
#[derive(Clone, PartialEq)]
pub struct Nutrients(pub [Option<f64>; N]);

impl fmt::Debug for Nutrients {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut m = f.debug_map();
        for (i, v) in self.0.iter().enumerate() {
            if let Some(v) = v {
                m.entry(&CATALOG[i].key, v);
            }
        }
        m.finish()
    }
}

impl Default for Nutrients {
    fn default() -> Self {
        Nutrients::empty()
    }
}

impl Nutrients {
    pub fn empty() -> Self {
        Nutrients([None; N])
    }

    pub fn get(&self, key: &str) -> Option<f64> {
        self.0[idx(key)]
    }

    pub fn val(&self, key: &str) -> f64 {
        self.get(key).unwrap_or(0.0)
    }

    pub fn set(&mut self, key: &str, v: Option<f64>) {
        self.0[idx(key)] = v;
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|v| v.is_none())
    }

    pub fn known_count(&self) -> usize {
        self.0.iter().filter(|v| v.is_some()).count()
    }

    pub fn scale(&self, factor: f64) -> Nutrients {
        let mut out = Nutrients::empty();
        for i in 0..N {
            out.0[i] = self.0[i].map(|v| v * factor);
        }
        out
    }

    /// Add `other` into self. Unknown + known = known, unknown + unknown = unknown.
    pub fn add(&mut self, other: &Nutrients) {
        for i in 0..N {
            self.0[i] = match (self.0[i], other.0[i]) {
                (None, None) => None,
                (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
            };
        }
    }

    /// Round for storage/display so float noise does not leak into the UI.
    pub fn rounded(&self) -> Nutrients {
        let mut out = Nutrients::empty();
        for i in 0..N {
            out.0[i] = self.0[i].map(|v| round_to(v, 4));
        }
        out
    }

    /// Read nutrient columns from a row starting at column `offset`.
    pub fn from_row(row: &rusqlite::Row, offset: usize) -> rusqlite::Result<Nutrients> {
        let mut out = Nutrients::empty();
        for (i, slot) in out.0.iter_mut().enumerate() {
            *slot = row.get::<_, Option<f64>>(offset + i)?;
        }
        Ok(out)
    }

    pub fn params(&self) -> Vec<Option<f64>> {
        self.0.to_vec()
    }

    /// Parse a JSON object of `{key or alias: number|null}`. Unknown keys and invalid values are errors
    /// whose message lists valid keys, so an AI client can correct itself.
    pub fn from_json(v: &serde_json::Value) -> Result<Nutrients, String> {
        let mut out = Nutrients::empty();
        let obj = match v {
            serde_json::Value::Null => return Ok(out),
            serde_json::Value::Object(o) => o,
            _ => {
                return Err(
                    "nutrients must be an object like {\"calories\": 120, \"protein\": 8}".into(),
                )
            }
        };
        let mut unknown = vec![];
        for (k, val) in obj {
            let Some(i) = resolve_key(k) else {
                unknown.push(k.clone());
                continue;
            };
            let num = match val {
                serde_json::Value::Null => None,
                serde_json::Value::Number(n) => n.as_f64(),
                serde_json::Value::String(s) if s.trim().is_empty() => None,
                serde_json::Value::String(s) => Some(
                    s.trim()
                        .trim_end_matches(|c: char| c.is_alphabetic() || c == 'µ')
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| format!("nutrient '{k}' must be a number, got '{s}'"))?,
                ),
                _ => return Err(format!("nutrient '{k}' must be a number")),
            };
            if let Some(x) = num {
                if !x.is_finite() || x < 0.0 {
                    return Err(format!(
                        "nutrient '{k}' must be a non-negative number, got {x}"
                    ));
                }
                if x > 1_000_000.0 {
                    return Err(format!("nutrient '{k}' value {x} is implausibly large"));
                }
            }
            out.0[i] = num;
        }
        if !unknown.is_empty() {
            return Err(format!(
                "unknown nutrient key(s): {}. Valid keys (with units): {}",
                unknown.join(", "),
                CATALOG
                    .iter()
                    .map(|n| format!("{} ({})", n.key, n.unit))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        Ok(out)
    }
}

impl Serialize for Nutrients {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(None)?;
        for (i, v) in self.0.iter().enumerate() {
            if let Some(v) = v {
                m.serialize_entry(CATALOG[i].key, &round_to(*v, 4))?;
            }
        }
        m.end()
    }
}

impl<'de> Deserialize<'de> for Nutrients {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        Nutrients::from_json(&v).map_err(serde::de::Error::custom)
    }
}

pub fn round_to(v: f64, places: i32) -> f64 {
    let p = 10f64.powi(places);
    (v * p).round() / p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_aliases_do_not_collide() {
        let mut seen = std::collections::HashSet::new();
        for n in CATALOG {
            assert!(seen.insert(n.key), "dup key {}", n.key);
            for a in n.aliases {
                assert!(seen.insert(a), "alias collides {a}");
            }
        }
    }

    #[test]
    fn resolves_aliases() {
        assert_eq!(resolve_key("kcal"), index_of("calories"));
        assert_eq!(resolve_key("Carbohydrates"), index_of("carbs"));
        assert_eq!(resolve_key("protein_g"), index_of("protein"));
        assert_eq!(resolve_key("vitamin-c"), index_of("vitamin_c"));
        assert_eq!(resolve_key("Vitamin C"), index_of("vitamin_c"));
        assert_eq!(resolve_key("sodium_mg"), index_of("sodium"));
        assert_eq!(resolve_key("vitamin_b12"), index_of("vitamin_b12"));
        assert_eq!(resolve_key("saturated fat"), index_of("saturated_fat"));
        assert_eq!(resolve_key("nonsense"), None);
    }

    #[test]
    fn json_round_trip_and_validation() {
        let n =
            Nutrients::from_json(&serde_json::json!({"kcal": 100, "protein": "8 g", "fat": null}))
                .unwrap();
        assert_eq!(n.get("calories"), Some(100.0));
        assert_eq!(n.get("protein"), Some(8.0));
        assert_eq!(n.get("fat"), None);
        let s = serde_json::to_value(&n).unwrap();
        assert_eq!(s, serde_json::json!({"calories": 100.0, "protein": 8.0}));
        let err = Nutrients::from_json(&serde_json::json!({"proteinz": 1})).unwrap_err();
        assert!(err.contains("proteinz") && err.contains("protein (g)"));
        assert!(Nutrients::from_json(&serde_json::json!({"protein": -1})).is_err());
    }

    #[test]
    fn add_keeps_unknown_distinct_from_zero() {
        let mut a = Nutrients::empty();
        a.set("protein", Some(2.0));
        let mut b = Nutrients::empty();
        b.set("protein", Some(3.0));
        b.set("iron", Some(1.0));
        a.add(&b);
        assert_eq!(a.get("protein"), Some(5.0));
        assert_eq!(a.get("iron"), Some(1.0));
        assert_eq!(a.get("calcium"), None);
    }
}
