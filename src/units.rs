//! Quantity units. Foods store nutrients per `basis_amount` of a basis unit (g, ml or serving);
//! a logged quantity is converted into that basis unit.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dim {
    Mass,
    Volume,
}

/// (canonical name, dimension, size in g or ml, accepted spellings)
const UNITS: &[(&str, Dim, f64, &[&str])] = &[
    (
        "g",
        Dim::Mass,
        1.0,
        &["g", "gram", "grams", "gm", "gms", "gr", "grm"],
    ),
    (
        "kg",
        Dim::Mass,
        1000.0,
        &["kg", "kgs", "kilogram", "kilograms", "kilo", "kilos"],
    ),
    ("mg", Dim::Mass, 0.001, &["mg", "milligram", "milligrams"]),
    (
        "oz",
        Dim::Mass,
        28.349523125,
        &["oz", "ounce", "ounces", "onz"],
    ),
    (
        "lb",
        Dim::Mass,
        453.59237,
        &["lb", "lbs", "pound", "pounds"],
    ),
    (
        "ml",
        Dim::Volume,
        1.0,
        &[
            "ml",
            "milliliter",
            "milliliters",
            "millilitre",
            "millilitres",
            "mll",
            "mlt",
            "cc",
        ],
    ),
    (
        "l",
        Dim::Volume,
        1000.0,
        &["l", "liter", "liters", "litre", "litres", "ltr"],
    ),
    ("dl", Dim::Volume, 100.0, &["dl", "deciliter", "decilitre"]),
    (
        "tsp",
        Dim::Volume,
        4.92892159375,
        &["tsp", "teaspoon", "teaspoons", "tsps"],
    ),
    (
        "tbsp",
        Dim::Volume,
        14.78676478125,
        &["tbsp", "tablespoon", "tablespoons", "tbs", "tbsps", "tbl"],
    ),
    (
        "fl oz",
        Dim::Volume,
        29.5735295625,
        &[
            "fl oz",
            "floz",
            "fl_oz",
            "fl. oz",
            "fluid ounce",
            "fluid ounces",
            "fl oz.",
        ],
    ),
    ("cup", Dim::Volume, 236.5882365, &["cup", "cups", "c"]),
    ("pint", Dim::Volume, 473.176473, &["pint", "pints", "pt"]),
    ("quart", Dim::Volume, 946.352946, &["quart", "quarts", "qt"]),
    (
        "gallon",
        Dim::Volume,
        3785.411784,
        &["gallon", "gallons", "gal"],
    ),
];

pub fn normalize(raw: &str) -> String {
    raw.trim()
        .to_lowercase()
        .trim_end_matches('.')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Look up a measurement unit: returns (canonical name, dimension, size in g/ml).
pub fn lookup(raw: &str) -> Option<(&'static str, Dim, f64)> {
    let u = normalize(raw);
    UNITS
        .iter()
        .find(|(_, _, _, names)| names.contains(&u.as_str()))
        .map(|(c, d, f, _)| (*c, *d, *f))
}

pub fn basis_dim(basis_unit: &str) -> Option<Dim> {
    match basis_unit {
        "g" => Some(Dim::Mass),
        "ml" => Some(Dim::Volume),
        _ => None,
    }
}

/// Convert `qty` of `unit` into the basis unit ("g" or "ml"). None when the unit is not a measurement
/// unit or the dimensions differ (mass vs volume).
pub fn to_basis(qty: f64, unit: &str, basis_unit: &str) -> Option<f64> {
    let dim = basis_dim(basis_unit)?;
    let (_, d, f) = lookup(unit)?;
    if d != dim {
        return None;
    }
    Some(qty * f)
}

/// Measurement units that can be used for a basis unit, for UI pickers and error messages.
pub fn compatible(basis_unit: &str) -> Vec<&'static str> {
    match basis_dim(basis_unit) {
        Some(dim) => UNITS
            .iter()
            .filter(|u| u.1 == dim && u.0 != "mg")
            .map(|u| u.0)
            .collect(),
        None => vec![],
    }
}

/// Normalize a serving label for matching: "1 Cup (240 ml)" -> "cup".
pub fn serving_key(label: &str) -> String {
    let mut s = normalize(label);
    if let Some(p) = s.find('(') {
        s.truncate(p);
    }
    let s = s.trim();
    let s = s.strip_prefix("1 ").unwrap_or(s).trim();
    let s = s.strip_suffix('s').filter(|x| x.len() > 2).unwrap_or(s);
    s.to_string()
}

pub const KG_PER_LB: f64 = 0.45359237;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions() {
        assert_eq!(
            to_basis(2.0, "oz", "g").map(|v| (v * 100.0).round() / 100.0),
            Some(56.7)
        );
        assert_eq!(to_basis(1.0, "Cups", "ml").map(|v| v.round()), Some(237.0));
        assert_eq!(
            to_basis(1.0, "fl oz", "ml").map(|v| (v * 10.0).round() / 10.0),
            Some(29.6)
        );
        assert_eq!(to_basis(1.0, "kg", "g"), Some(1000.0));
        assert_eq!(
            to_basis(1.0, "cup", "g"),
            None,
            "no density, no mass/volume conversion"
        );
        assert_eq!(to_basis(1.0, "g", "serving"), None);
        assert_eq!(to_basis(1.0, "bowl", "g"), None);
    }

    #[test]
    fn serving_keys() {
        assert_eq!(serving_key("1 Cup"), "cup");
        assert_eq!(serving_key("cups"), "cup");
        assert_eq!(serving_key("1 bottle (591 ml)"), "bottle");
        assert_eq!(serving_key("Slice"), "slice");
        assert_eq!(serving_key("11 pieces"), "11 piece");
    }
}
