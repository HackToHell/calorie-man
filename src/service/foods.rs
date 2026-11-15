use crate::error::{bad, not_found, AppError, AppResult};
use crate::nutrients::{self, Nutrients, CATALOG};
use crate::units::{self, compatible, lookup, normalize, serving_key, to_basis};
use crate::util::clean_opt;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize)]
pub struct Serving {
    pub id: i64,
    pub label: String,
    /// Size of the serving in the food's basis unit (g, ml or servings).
    pub amount: f64,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Ingredient {
    pub id: i64,
    pub food_id: i64,
    pub name: String,
    pub brand: Option<String>,
    pub quantity: f64,
    pub unit: String,
    pub serving_id: Option<i64>,
    pub amount: f64,
    pub basis_unit: String,
    pub nutrients: Nutrients,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerServing {
    pub label: String,
    pub amount: f64,
    pub calories: Option<f64>,
    pub protein: Option<f64>,
    pub carbs: Option<f64>,
    pub fat: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Food {
    pub id: i64,
    pub name: String,
    pub brand: Option<String>,
    pub kind: String,
    /// Nutrients are given per `basis_amount` of `basis_unit`.
    pub basis_amount: f64,
    pub basis_unit: String,
    pub barcode: Option<String>,
    pub source: String,
    pub source_id: Option<String>,
    pub aliases: Option<String>,
    pub notes: Option<String>,
    pub recipe_yield: Option<f64>,
    pub archived: bool,
    pub created_by: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub nutrients: Nutrients,
    pub servings: Vec<Serving>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingredients: Option<Vec<Ingredient>>,
    /// Measurement units accepted when logging this food (in addition to its servings).
    pub units: Vec<&'static str>,
    pub per_serving: Option<PerServing>,
}

impl Food {
    pub fn default_serving(&self) -> Option<&Serving> {
        self.servings
            .iter()
            .find(|s| s.is_default)
            .or(self.servings.first())
    }

    pub fn nutrients_for(&self, amount: f64) -> Nutrients {
        self.nutrients.scale(amount / self.basis_amount).rounded()
    }

    pub fn basis_label(&self) -> String {
        match self.basis_unit.as_str() {
            "serving" if self.basis_amount == 1.0 => "1 serving".into(),
            "serving" => format!("{} servings", fmt_num(self.basis_amount)),
            u => format!("{} {u}", fmt_num(self.basis_amount)),
        }
    }
}

pub fn fmt_num(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServingInput {
    pub id: Option<i64>,
    pub label: String,
    /// Size in `unit`, or in the food's basis unit when `unit` is omitted.
    pub amount: f64,
    pub unit: Option<String>,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngredientInput {
    pub food_id: i64,
    pub quantity: f64,
    pub unit: Option<String>,
    pub serving_id: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoodInput {
    pub name: String,
    pub brand: Option<String>,
    pub kind: Option<String>,
    pub basis_amount: Option<f64>,
    pub basis_unit: Option<String>,
    pub barcode: Option<String>,
    pub aliases: Option<String>,
    pub notes: Option<String>,
    #[serde(default)]
    pub nutrients: Nutrients,
    pub servings: Option<Vec<ServingInput>>,
    pub source: Option<String>,
    pub source_id: Option<String>,
    /// Recipes only.
    pub ingredients: Option<Vec<IngredientInput>>,
    pub recipe_yield: Option<f64>,
    pub cooked_weight_g: Option<f64>,
}

const FOOD_COLS: &str =
    "id, name, brand, kind, basis_amount, basis_unit, barcode, source, source_id, aliases, notes, \
     recipe_yield, archived, created_by, created_at, updated_at";
const N_OFFSET: usize = 16;

fn select_sql(prefix: &str) -> String {
    let cols = FOOD_COLS
        .split(", ")
        .map(|c| format!("{prefix}{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "SELECT {cols}, {} FROM foods {}",
        nutrients::columns(prefix),
        prefix.trim_end_matches('.')
    )
}

fn from_row(r: &Row) -> rusqlite::Result<Food> {
    let basis_unit: String = r.get(5)?;
    Ok(Food {
        id: r.get(0)?,
        name: r.get(1)?,
        brand: r.get(2)?,
        kind: r.get(3)?,
        basis_amount: r.get(4)?,
        units: units_for(&basis_unit),
        basis_unit,
        barcode: r.get(6)?,
        source: r.get(7)?,
        source_id: r.get(8)?,
        aliases: r.get(9)?,
        notes: r.get(10)?,
        recipe_yield: r.get(11)?,
        archived: r.get::<_, i64>(12)? != 0,
        created_by: r.get(13)?,
        created_at: r.get(14)?,
        updated_at: r.get(15)?,
        nutrients: Nutrients::from_row(r, N_OFFSET)?,
        servings: vec![],
        ingredients: None,
        per_serving: None,
    })
}

fn units_for(basis_unit: &str) -> Vec<&'static str> {
    if basis_unit == "serving" {
        vec!["serving"]
    } else {
        compatible(basis_unit)
    }
}

fn load_servings(c: &Connection, food_id: i64) -> AppResult<Vec<Serving>> {
    let mut st = c.prepare_cached(
        "SELECT id, label, amount, is_default FROM food_servings WHERE food_id = ? ORDER BY position, id",
    )?;
    let rows = st.query_map([food_id], |r| {
        Ok(Serving {
            id: r.get(0)?,
            label: r.get(1)?,
            amount: r.get(2)?,
            is_default: r.get::<_, i64>(3)? != 0,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn load_ingredients(c: &Connection, recipe_id: i64) -> AppResult<Vec<Ingredient>> {
    let mut st = c.prepare_cached(
        "SELECT ri.id, ri.food_id, f.name, f.brand, ri.quantity, ri.unit, ri.serving_id, ri.amount, f.basis_unit, f.basis_amount
         FROM recipe_items ri JOIN foods f ON f.id = ri.food_id WHERE ri.recipe_id = ? ORDER BY ri.position, ri.id",
    )?;
    let rows = st.query_map([recipe_id], |r| {
        Ok((
            Ingredient {
                id: r.get(0)?,
                food_id: r.get(1)?,
                name: r.get(2)?,
                brand: r.get(3)?,
                quantity: r.get(4)?,
                unit: r.get(5)?,
                serving_id: r.get(6)?,
                amount: r.get(7)?,
                basis_unit: r.get(8)?,
                nutrients: Nutrients::empty(),
            },
            r.get::<_, f64>(9)?,
        ))
    })?;
    let mut out = vec![];
    for row in rows {
        let (mut ing, _) = row?;
        let food = get_bare(c, ing.food_id)?;
        ing.nutrients = food.nutrients_for(ing.amount);
        out.push(ing);
    }
    Ok(out)
}

fn finish(c: &Connection, mut f: Food) -> AppResult<Food> {
    f.servings = load_servings(c, f.id)?;
    if f.kind == "recipe" {
        f.ingredients = Some(load_ingredients(c, f.id)?);
    }
    f.per_serving = f.default_serving().map(|s| {
        let n = f.nutrients_for(s.amount);
        PerServing {
            label: s.label.clone(),
            amount: s.amount,
            calories: n.get("calories"),
            protein: n.get("protein"),
            carbs: n.get("carbs"),
            fat: n.get("fat"),
        }
    });
    Ok(f)
}

fn get_bare(c: &Connection, id: i64) -> AppResult<Food> {
    c.query_row(&format!("{} WHERE id = ?", select_sql("")), [id], from_row)
        .optional()?
        .ok_or_else(|| not_found(format!("food {id} not found")))
}

pub fn get(c: &Connection, id: i64) -> AppResult<Food> {
    let f = get_bare(c, id)?;
    finish(c, f)
}

pub fn find_by_barcode(c: &Connection, code: &str) -> AppResult<Option<Food>> {
    let code = normalize_barcode(code);
    if code.is_empty() {
        return Ok(None);
    }
    let trimmed = code.trim_start_matches('0').to_string();
    let f = c
        .query_row(
            &format!(
                "{} WHERE archived = 0 AND ltrim(barcode, '0') = ? ORDER BY id LIMIT 1",
                select_sql("")
            ),
            [&trimmed],
            from_row,
        )
        .optional()?;
    f.map(|f| finish(c, f)).transpose()
}

pub fn find_by_source(c: &Connection, source: &str, source_id: &str) -> AppResult<Option<Food>> {
    let f = c
        .query_row(
            &format!("{} WHERE source = ? AND source_id = ?", select_sql("")),
            [source, source_id],
            from_row,
        )
        .optional()?;
    f.map(|f| finish(c, f)).transpose()
}

pub fn normalize_barcode(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

// ---------------------------------------------------------------------------------------------------------
// Quantities

#[derive(Debug, Clone)]
pub struct Resolved {
    /// Amount in the food's basis unit.
    pub amount: f64,
    /// Display unit: a serving label or a measurement unit.
    pub unit: String,
    pub serving_id: Option<i64>,
}

fn unit_help(f: &Food) -> String {
    let mut opts: Vec<String> = f.units.iter().map(|u| u.to_string()).collect();
    for s in &f.servings {
        opts.push(format!(
            "'{}' (serving_id {}, = {} {})",
            s.label,
            s.id,
            fmt_num(s.amount),
            f.basis_unit
        ));
    }
    if f.basis_unit != "serving" && f.default_serving().is_some() {
        opts.push("serving (the default serving)".into());
    }
    opts.join(", ")
}

/// Turn "2 cups", "150 g", "1 serving" or a serving id into an amount of the food's basis unit.
pub fn resolve_amount(
    f: &Food,
    qty: f64,
    unit: Option<&str>,
    serving_id: Option<i64>,
) -> AppResult<Resolved> {
    if !qty.is_finite() || qty <= 0.0 || qty > 100_000.0 {
        return Err(bad("quantity must be a positive number"));
    }
    if let Some(sid) = serving_id {
        let s = f.servings.iter().find(|s| s.id == sid).ok_or_else(|| {
            bad(format!(
                "serving_id {sid} does not belong to '{}'. Options: {}",
                f.name,
                unit_help(f)
            ))
        })?;
        return Ok(Resolved {
            amount: qty * s.amount,
            unit: s.label.clone(),
            serving_id: Some(s.id),
        });
    }
    let unit = unit.map(str::trim).filter(|u| !u.is_empty());
    let Some(u) = unit else {
        if let Some(s) = f.default_serving() {
            return Ok(Resolved {
                amount: qty * s.amount,
                unit: s.label.clone(),
                serving_id: Some(s.id),
            });
        }
        let label = if f.basis_unit == "serving" {
            "serving".to_string()
        } else {
            f.basis_unit.clone()
        };
        return Ok(Resolved {
            amount: qty,
            unit: label,
            serving_id: None,
        });
    };
    let nu = normalize(u);
    if matches!(
        nu.as_str(),
        "serving" | "servings" | "srv" | "portion" | "portions" | "x"
    ) {
        if let Some(s) = f.default_serving() {
            return Ok(Resolved {
                amount: qty * s.amount,
                unit: s.label.clone(),
                serving_id: Some(s.id),
            });
        }
        if f.basis_unit == "serving" {
            return Ok(Resolved {
                amount: qty,
                unit: "serving".into(),
                serving_id: None,
            });
        }
        return Err(bad(format!(
            "'{}' has no servings defined; log it by weight/volume. Options: {}",
            f.name,
            unit_help(f)
        )));
    }
    let key = serving_key(u);
    if let Some(s) = f
        .servings
        .iter()
        .find(|s| serving_key(&s.label) == key || normalize(&s.label) == nu)
    {
        return Ok(Resolved {
            amount: qty * s.amount,
            unit: s.label.clone(),
            serving_id: Some(s.id),
        });
    }
    if let Some(amount) = to_basis(qty, u, &f.basis_unit) {
        let canonical = lookup(u).map(|x| x.0).unwrap_or(u);
        return Ok(Resolved {
            amount,
            unit: canonical.to_string(),
            serving_id: None,
        });
    }
    Err(bad(format!(
        "unit '{u}' does not work for '{}'. Options: {}",
        f.name,
        unit_help(f)
    )))
}

// ---------------------------------------------------------------------------------------------------------
// Create / update

const SOURCES: &[&str] = &["manual", "ai", "usda", "off", "recipe"];

fn normalize_basis(amount: Option<f64>, unit: Option<&str>) -> AppResult<(f64, String)> {
    let unit = unit.map(str::trim).filter(|u| !u.is_empty()).unwrap_or("g");
    let nu = normalize(unit);
    if matches!(nu.as_str(), "serving" | "servings" | "portion") {
        let a = amount.unwrap_or(1.0);
        crate::util::finite_pos(a, "basis_amount")?;
        return Ok((a, "serving".into()));
    }
    let (_, dim, factor) = lookup(unit).ok_or_else(|| {
        bad(format!(
            "basis_unit '{unit}' must be g, ml, serving or a weight/volume unit"
        ))
    })?;
    let a = amount.unwrap_or(100.0);
    crate::util::finite_pos(a, "basis_amount")?;
    let base = match dim {
        units::Dim::Mass => "g",
        units::Dim::Volume => "ml",
    };
    Ok((a * factor, base.into()))
}

fn sanity(
    name: &str,
    n: &Nutrients,
    basis_amount: f64,
    basis_unit: &str,
) -> AppResult<Vec<String>> {
    let mut warn = vec![];
    if basis_unit == "g" {
        let per100 = 100.0 / basis_amount;
        let macros = (n.val("protein") + n.val("carbs") + n.val("fat")) * per100;
        if macros > 105.0 {
            return Err(bad(format!(
                "protein + carbs + fat = {:.1} g per 100 g for '{name}', which is impossible. Check the basis \
                 (nutrients are per {} g).",
                macros,
                fmt_num(basis_amount)
            )));
        }
        if let Some(cal) = n.get("calories") {
            if cal * per100 > 950.0 {
                return Err(bad(format!(
                    "{:.0} kcal per 100 g for '{name}' is impossible (pure fat is ~900). Check the basis.",
                    cal * per100
                )));
            }
        }
    }
    match n.get("calories") {
        None => warn.push("no calories given; this food will count as 0 kcal".into()),
        Some(cal) => {
            if n.get("protein").is_some() || n.get("carbs").is_some() || n.get("fat").is_some() {
                let est = 4.0 * n.val("protein")
                    + 4.0 * n.val("carbs")
                    + 9.0 * n.val("fat")
                    + 7.0 * n.val("alcohol");
                if (cal - est).abs() > f64::max(25.0 * basis_amount / 100.0, 0.25 * cal.max(est))
                    && (cal - est).abs() > 10.0
                {
                    warn.push(format!(
                        "calories ({cal:.0}) do not match the macros (about {est:.0} kcal from 4/4/9); double-check the values"
                    ));
                }
            }
        }
    }
    Ok(warn)
}

fn check_text(v: Option<String>, what: &str, max: usize) -> AppResult<Option<String>> {
    let v = clean_opt(v);
    if let Some(s) = &v {
        if s.chars().count() > max {
            return Err(bad(format!("{what} is too long (max {max} characters)")));
        }
    }
    Ok(v)
}

fn prepare_servings(inp: &[ServingInput], basis_unit: &str) -> AppResult<Vec<ServingInput>> {
    let mut out: Vec<ServingInput> = vec![];
    for s in inp {
        let label = s.label.trim().to_string();
        if label.is_empty() || label.chars().count() > 60 {
            return Err(bad("serving label must be 1-60 characters"));
        }
        crate::util::finite_pos(s.amount, &format!("serving '{label}' amount"))?;
        let amount = match s.unit.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
            None => s.amount,
            Some(u) if basis_unit == "serving" && matches!(normalize(u).as_str(), "serving" | "servings") => s.amount,
            Some(u) => to_basis(s.amount, u, basis_unit).ok_or_else(|| {
                bad(format!(
                    "serving '{label}': unit '{u}' cannot be converted to the food's basis unit '{basis_unit}' \
                     (mass and volume do not mix; give the amount in {basis_unit})"
                ))
            })?,
        };
        if out.iter().any(|o| o.label.eq_ignore_ascii_case(&label)) {
            return Err(bad(format!("duplicate serving label '{label}'")));
        }
        out.push(ServingInput {
            id: s.id,
            label,
            amount,
            unit: None,
            is_default: s.is_default,
        });
    }
    if !out.is_empty() && !out.iter().any(|s| s.is_default) {
        out[0].is_default = true;
    }
    let mut seen_default = false;
    for s in out.iter_mut() {
        if s.is_default {
            if seen_default {
                s.is_default = false;
            }
            seen_default = true;
        }
    }
    Ok(out)
}

fn save_servings(c: &Connection, food_id: i64, servings: &[ServingInput]) -> AppResult<()> {
    let existing = load_servings(c, food_id)?;
    let mut keep: HashSet<i64> = HashSet::new();
    for (pos, s) in servings.iter().enumerate() {
        let match_id =
            s.id.filter(|id| existing.iter().any(|e| e.id == *id))
                .or_else(|| {
                    existing
                        .iter()
                        .find(|e| e.label.eq_ignore_ascii_case(&s.label))
                        .map(|e| e.id)
                })
                .filter(|id| !keep.contains(id));
        match match_id {
            Some(id) => {
                c.execute(
                    "UPDATE food_servings SET label=?1, amount=?2, is_default=?3, position=?4 WHERE id=?5",
                    params![s.label, s.amount, s.is_default as i64, pos as i64, id],
                )?;
                keep.insert(id);
            }
            None => {
                c.execute(
                    "INSERT INTO food_servings (food_id, label, amount, is_default, position) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![food_id, s.label, s.amount, s.is_default as i64, pos as i64],
                )?;
                keep.insert(c.last_insert_rowid());
            }
        }
    }
    for e in existing {
        if !keep.contains(&e.id) {
            c.execute("DELETE FROM food_servings WHERE id = ?", [e.id])?;
        }
    }
    Ok(())
}

struct BuiltRecipe {
    total: Nutrients,
    items: Vec<(IngredientInput, Resolved)>,
}

/// Does `root` (or anything inside it, if it is a recipe) contain `target`?
fn contains(c: &Connection, root: i64, target: i64, depth: usize) -> AppResult<bool> {
    if root == target {
        return Ok(true);
    }
    if depth > 20 {
        return Err(bad("recipes are nested too deeply"));
    }
    let children: Vec<i64> = {
        let mut st =
            c.prepare_cached("SELECT DISTINCT food_id FROM recipe_items WHERE recipe_id = ?")?;
        let rows = st.query_map([root], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    for ch in children {
        if contains(c, ch, target, depth + 1)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn build_recipe(
    c: &Connection,
    recipe_id: Option<i64>,
    items: &[IngredientInput],
) -> AppResult<BuiltRecipe> {
    if items.is_empty() {
        return Err(bad("a recipe needs at least one ingredient"));
    }
    if items.len() > 100 {
        return Err(bad("a recipe can have at most 100 ingredients"));
    }
    let mut total = Nutrients::empty();
    let mut out = vec![];
    for it in items {
        let food = get(c, it.food_id)
            .map_err(|_| bad(format!("ingredient food_id {} does not exist", it.food_id)))?;
        if let Some(rid) = recipe_id {
            if contains(c, food.id, rid, 0)? {
                return Err(bad(format!(
                    "'{}' contains this recipe, so it cannot be an ingredient",
                    food.name
                )));
            }
        }
        let r = resolve_amount(&food, it.quantity, it.unit.as_deref(), it.serving_id)?;
        total.add(&food.nutrients.scale(r.amount / food.basis_amount));
        out.push((it.clone(), r));
    }
    Ok(BuiltRecipe {
        total: total.rounded(),
        items: out,
    })
}

fn recipe_basis(yield_: f64, cooked_weight_g: Option<f64>) -> (f64, String, Vec<ServingInput>) {
    let mk = |label: &str, amount: f64, d: bool| ServingInput {
        id: None,
        label: label.into(),
        amount,
        unit: None,
        is_default: d,
    };
    match cooked_weight_g {
        Some(w) => (
            w,
            "g".into(),
            vec![
                mk("serving", w / yield_, true),
                mk("whole recipe", w, false),
            ],
        ),
        None => {
            let mut s = vec![mk("serving", 1.0, true)];
            if yield_ != 1.0 {
                s.push(mk("whole recipe", yield_, false));
            }
            (yield_, "serving".into(), s)
        }
    }
}

fn save_items(
    c: &Connection,
    recipe_id: i64,
    items: &[(IngredientInput, Resolved)],
) -> AppResult<()> {
    c.execute("DELETE FROM recipe_items WHERE recipe_id = ?", [recipe_id])?;
    for (pos, (it, r)) in items.iter().enumerate() {
        c.execute(
            "INSERT INTO recipe_items (recipe_id, food_id, quantity, unit, serving_id, amount, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![recipe_id, it.food_id, it.quantity, r.unit, r.serving_id, r.amount, pos as i64],
        )?;
    }
    Ok(())
}

fn write_nutrients(c: &Connection, table: &str, id: i64, n: &Nutrients) -> AppResult<()> {
    let sets = CATALOG
        .iter()
        .enumerate()
        .map(|(i, x)| format!("n_{} = ?{}", x.key, i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let mut p: Vec<rusqlite::types::Value> = n.params().into_iter().map(|v| v.into()).collect();
    p.push(id.into());
    c.execute(
        &format!(
            "UPDATE {table} SET {sets} WHERE id = ?{}",
            CATALOG.len() + 1
        ),
        params_from_iter(p),
    )?;
    Ok(())
}

pub struct Saved {
    pub food: Food,
    pub warnings: Vec<String>,
    pub entries_updated: usize,
}

struct Prepared {
    name: String,
    brand: Option<String>,
    kind: String,
    basis_amount: f64,
    basis_unit: String,
    barcode: Option<String>,
    aliases: Option<String>,
    notes: Option<String>,
    source: String,
    source_id: Option<String>,
    nutrients: Nutrients,
    servings: Vec<ServingInput>,
    recipe: Option<(f64, Vec<(IngredientInput, Resolved)>)>,
    warnings: Vec<String>,
}

fn prepare(c: &Connection, inp: FoodInput, existing_id: Option<i64>) -> AppResult<Prepared> {
    let name = inp.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 200 {
        return Err(bad("food name must be 1-200 characters"));
    }
    let brand = check_text(inp.brand, "brand", 100)?;
    let aliases = check_text(inp.aliases, "aliases", 500)?;
    let notes = check_text(inp.notes, "notes", 2000)?;
    let source = clean_opt(inp.source)
        .unwrap_or_else(|| "manual".into())
        .to_lowercase();
    let is_recipe = inp.kind.as_deref() == Some("recipe") || inp.ingredients.is_some();
    if let Some(k) = inp.kind.as_deref() {
        if k != "food" && k != "recipe" {
            return Err(bad("kind must be 'food' or 'recipe'"));
        }
    }
    if !SOURCES.contains(&source.as_str()) {
        return Err(bad(format!("source must be one of {}", SOURCES.join(", "))));
    }
    let barcode = inp
        .barcode
        .as_deref()
        .map(normalize_barcode)
        .filter(|b| !b.is_empty());
    if let Some(b) = &barcode {
        if b.len() < 6 || b.len() > 14 {
            return Err(bad("barcode must have 6-14 digits"));
        }
        if let Some(other) = find_by_barcode(c, b)? {
            if Some(other.id) != existing_id {
                return Err(AppError::Conflict(format!(
                    "barcode {b} already belongs to food {} '{}'; use or update that food instead",
                    other.id, other.name
                )));
            }
        }
    }
    if is_recipe {
        let items = inp
            .ingredients
            .ok_or_else(|| bad("a recipe needs ingredients"))?;
        let y = inp.recipe_yield.unwrap_or(1.0);
        if !y.is_finite() || y <= 0.0 || y > 1000.0 {
            return Err(bad(
                "recipe_yield (number of servings) must be between 0 and 1000",
            ));
        }
        if let Some(w) = inp.cooked_weight_g {
            if !w.is_finite() || w <= 0.0 || w > 100_000.0 {
                return Err(bad("cooked_weight_g must be a positive number of grams"));
            }
        }
        let built = build_recipe(c, existing_id, &items)?;
        let (basis_amount, basis_unit, servings) = recipe_basis(y, inp.cooked_weight_g);
        let mut warnings = vec![];
        if built.total.get("calories").is_none() {
            warnings.push("none of the ingredients have calories".into());
        }
        return Ok(Prepared {
            name,
            brand,
            kind: "recipe".into(),
            basis_amount,
            basis_unit: basis_unit.clone(),
            barcode,
            aliases,
            notes,
            source: "recipe".into(),
            source_id: None,
            nutrients: built.total,
            servings: prepare_servings(&servings, &basis_unit)?,
            recipe: Some((y, built.items)),
            warnings,
        });
    }
    let (basis_amount, basis_unit) = normalize_basis(inp.basis_amount, inp.basis_unit.as_deref())?;
    let warnings = sanity(&name, &inp.nutrients, basis_amount, &basis_unit)?;
    let servings = prepare_servings(inp.servings.as_deref().unwrap_or(&[]), &basis_unit)?;
    Ok(Prepared {
        name,
        brand,
        kind: "food".into(),
        basis_amount,
        basis_unit,
        barcode,
        aliases,
        notes,
        source,
        source_id: clean_opt(inp.source_id),
        nutrients: inp.nutrients.rounded(),
        servings,
        recipe: None,
        warnings,
    })
}

pub fn create(c: &mut Connection, inp: FoodInput, created_by: Option<i64>) -> AppResult<Saved> {
    let tx = c.transaction()?;
    let p = prepare(&tx, inp, None)?;
    if let Some(sid) = &p.source_id {
        if let Some(f) = find_by_source(&tx, &p.source, sid)? {
            return Err(AppError::Conflict(format!(
                "{} item {sid} is already in the library as food {} '{}'",
                p.source, f.id, f.name
            )));
        }
    }
    tx.execute(
        "INSERT INTO foods (name, brand, kind, basis_amount, basis_unit, barcode, source, source_id, aliases, notes,
           recipe_yield, created_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            p.name,
            p.brand,
            p.kind,
            p.basis_amount,
            p.basis_unit,
            p.barcode,
            p.source,
            p.source_id,
            p.aliases,
            p.notes,
            p.recipe.as_ref().map(|r| r.0),
            created_by
        ],
    )?;
    let id = tx.last_insert_rowid();
    write_nutrients(&tx, "foods", id, &p.nutrients)?;
    save_servings(&tx, id, &p.servings)?;
    if let Some((_, items)) = &p.recipe {
        save_items(&tx, id, items)?;
    }
    let food = get(&tx, id)?;
    tx.commit()?;
    Ok(Saved {
        food,
        warnings: p.warnings,
        entries_updated: 0,
    })
}

/// Replace a food. With `propagate`, past diary entries of this food (and of recipes using it) are
/// recalculated; recipes that use it are always recalculated.
pub fn update(
    c: &mut Connection,
    id: i64,
    mut inp: FoodInput,
    propagate: bool,
) -> AppResult<Saved> {
    let tx = c.transaction()?;
    let old = get(&tx, id)?;
    if old.kind == "recipe" {
        inp.kind = Some("recipe".into());
        if inp.ingredients.is_none() {
            inp.ingredients = old.ingredients.as_ref().map(|items| {
                items
                    .iter()
                    .map(|i| IngredientInput {
                        food_id: i.food_id,
                        quantity: i.quantity,
                        unit: Some(i.unit.clone()),
                        serving_id: i.serving_id,
                    })
                    .collect()
            });
        }
        if inp.recipe_yield.is_none() {
            inp.recipe_yield = old.recipe_yield;
        }
    } else if inp.kind.as_deref() == Some("recipe") || inp.ingredients.is_some() {
        return Err(bad(
            "a food cannot be turned into a recipe; create a recipe instead",
        ));
    }
    let p = prepare(&tx, inp, Some(id))?;
    let source = if old.source != "manual" && p.source == "manual" {
        old.source.clone()
    } else {
        p.source.clone()
    };
    let source_id = p.source_id.clone().or(old.source_id.clone());
    tx.execute(
        "UPDATE foods SET name=?1, brand=?2, basis_amount=?3, basis_unit=?4, barcode=?5, source=?6, source_id=?7,
           aliases=?8, notes=?9, recipe_yield=?10, updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id=?11",
        params![
            p.name,
            p.brand,
            p.basis_amount,
            p.basis_unit,
            p.barcode,
            source,
            source_id,
            p.aliases,
            p.notes,
            p.recipe.as_ref().map(|r| r.0),
            id
        ],
    )?;
    write_nutrients(&tx, "foods", id, &p.nutrients)?;
    save_servings(&tx, id, &p.servings)?;
    if let Some((_, items)) = &p.recipe {
        save_items(&tx, id, items)?;
    }
    let mut updated = 0;
    if propagate {
        updated += refresh_entries(&tx, id)?;
    }
    let mut visited = HashSet::from([id]);
    updated += recompute_dependents(&tx, id, propagate, &mut visited)?;
    let food = get(&tx, id)?;
    tx.commit()?;
    Ok(Saved {
        food,
        warnings: p.warnings,
        entries_updated: updated,
    })
}

/// Recalculate diary entries that point at `food_id` from the food's current values.
pub fn refresh_entries(c: &Connection, food_id: i64) -> AppResult<usize> {
    let food = get(c, food_id)?;
    let rows: Vec<(i64, f64, String, Option<i64>)> = {
        let mut st =
            c.prepare("SELECT id, quantity, unit, serving_id FROM entries WHERE food_id = ?")?;
        let rows = st.query_map([food_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?;
        rows.collect::<Result<_, _>>()?
    };
    let mut n = 0;
    for (eid, qty, unit, sid) in rows {
        let r = match resolve_amount(&food, qty, Some(&unit), sid)
            .or_else(|_| resolve_amount(&food, qty, Some(&unit), None))
        {
            Ok(r) => r,
            Err(_) => continue,
        };
        c.execute(
            "UPDATE entries SET name=?1, brand=?2, amount=?3, unit=?4, serving_id=?5 WHERE id=?6",
            params![food.name, food.brand, r.amount, r.unit, r.serving_id, eid],
        )?;
        write_nutrients(c, "entries", eid, &food.nutrients_for(r.amount))?;
        n += 1;
    }
    Ok(n)
}

fn recompute_dependents(
    c: &Connection,
    food_id: i64,
    propagate: bool,
    visited: &mut HashSet<i64>,
) -> AppResult<usize> {
    let recipes: Vec<i64> = {
        let mut st = c.prepare("SELECT DISTINCT recipe_id FROM recipe_items WHERE food_id = ?")?;
        let rows = st.query_map([food_id], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut n = 0;
    for rid in recipes {
        if !visited.insert(rid) {
            continue;
        }
        recompute_recipe(c, rid)?;
        if propagate {
            n += refresh_entries(c, rid)?;
        }
        n += recompute_dependents(c, rid, propagate, visited)?;
    }
    Ok(n)
}

fn recompute_recipe(c: &Connection, rid: i64) -> AppResult<()> {
    let items: Vec<(i64, i64, f64, String, Option<i64>, f64)> = {
        let mut st =
            c.prepare("SELECT id, food_id, quantity, unit, serving_id, amount FROM recipe_items WHERE recipe_id = ?")?;
        let rows = st.query_map([rid], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?;
        rows.collect::<Result<_, _>>()?
    };
    let mut total = Nutrients::empty();
    for (iid, fid, qty, unit, sid, amount) in items {
        let food = get(c, fid)?;
        let amount = match resolve_amount(&food, qty, Some(&unit), sid)
            .or_else(|_| resolve_amount(&food, qty, Some(&unit), None))
        {
            Ok(r) => {
                c.execute(
                    "UPDATE recipe_items SET amount=?1, serving_id=?2 WHERE id=?3",
                    params![r.amount, r.serving_id, iid],
                )?;
                r.amount
            }
            Err(_) => amount,
        };
        total.add(&food.nutrients.scale(amount / food.basis_amount));
    }
    write_nutrients(c, "foods", rid, &total.rounded())?;
    c.execute(
        "UPDATE foods SET updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id=?",
        [rid],
    )?;
    Ok(())
}

/// Delete a food. Foods that are used by entries or recipes are archived instead (hidden from search
/// but history and recipes keep working). Returns true when archived.
pub fn delete(c: &Connection, id: i64) -> AppResult<bool> {
    get_bare(c, id)?;
    let used: i64 = c.query_row(
        "SELECT (SELECT COUNT(*) FROM entries WHERE food_id = ?1) + (SELECT COUNT(*) FROM recipe_items WHERE food_id = ?1)",
        [id],
        |r| r.get(0),
    )?;
    if used > 0 {
        c.execute("UPDATE foods SET archived = 1 WHERE id = ?", [id])?;
        Ok(true)
    } else {
        c.execute("DELETE FROM foods WHERE id = ?", [id])?;
        Ok(false)
    }
}

pub fn set_archived(c: &Connection, id: i64, archived: bool) -> AppResult<Food> {
    get_bare(c, id)?;
    c.execute(
        "UPDATE foods SET archived = ? WHERE id = ?",
        params![archived as i64, id],
    )?;
    get(c, id)
}

pub fn to_input(f: &Food) -> FoodInput {
    FoodInput {
        name: f.name.clone(),
        brand: f.brand.clone(),
        kind: Some(f.kind.clone()),
        basis_amount: Some(f.basis_amount),
        basis_unit: Some(f.basis_unit.clone()),
        barcode: f.barcode.clone(),
        aliases: f.aliases.clone(),
        notes: f.notes.clone(),
        nutrients: f.nutrients.clone(),
        servings: if f.kind == "recipe" {
            None
        } else {
            Some(
                f.servings
                    .iter()
                    .map(|s| ServingInput {
                        id: Some(s.id),
                        label: s.label.clone(),
                        amount: s.amount,
                        unit: None,
                        is_default: s.is_default,
                    })
                    .collect(),
            )
        },
        source: Some(f.source.clone()),
        source_id: f.source_id.clone(),
        ingredients: f.ingredients.as_ref().map(|items| {
            items
                .iter()
                .map(|i| IngredientInput {
                    food_id: i.food_id,
                    quantity: i.quantity,
                    unit: Some(i.unit.clone()),
                    serving_id: i.serving_id,
                })
                .collect()
        }),
        recipe_yield: f.recipe_yield,
        cooked_weight_g: if f.kind == "recipe" && f.basis_unit == "g" {
            Some(f.basis_amount)
        } else {
            None
        },
    }
}

// ---------------------------------------------------------------------------------------------------------
// Search

/// Store brands that are sold under another name.
pub const SYNONYMS: &[(&str, &str)] = &[
    ("costco", "kirkland"),
    ("kirkland", "costco"),
    ("tj", "trader joe"),
];

pub fn expand_token(t: &str) -> Vec<String> {
    let mut v = vec![t.to_string()];
    for (a, b) in SYNONYMS {
        if t == *a {
            v.push(b.to_string());
        }
    }
    v
}

pub fn tokens(q: &str) -> Vec<String> {
    q.to_lowercase()
        .split(|c: char| c.is_whitespace() || c == ',' || c == '/')
        .map(|t| {
            t.trim_matches(|c: char| !c.is_alphanumeric() && c != '%' && c != '\'')
                .to_string()
        })
        .filter(|t| !t.is_empty())
        .take(8)
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    #[serde(flatten)]
    pub food: Food,
    pub uses: i64,
}

pub struct SearchOpts<'a> {
    pub q: &'a str,
    pub kind: Option<&'a str>,
    pub limit: usize,
    pub user_id: Option<i64>,
    pub include_archived: bool,
}

/// Search the shared library. Every word must match the name, brand, aliases or barcode ("costco" also
/// matches "kirkland"). Results are ranked by match quality and how often the foods were logged.
pub fn search(c: &Connection, o: SearchOpts) -> AppResult<Vec<SearchHit>> {
    let toks = tokens(o.q);
    let mut wh = vec![];
    let mut p: Vec<rusqlite::types::Value> = vec![];
    if !o.include_archived {
        wh.push("f.archived = 0".to_string());
    }
    if let Some(k) = o.kind {
        wh.push("f.kind = ?".into());
        p.push(k.to_string().into());
    }
    let hay = "lower(f.name || ' ' || coalesce(f.brand, '') || ' ' || coalesce(f.aliases, '') || ' ' || coalesce(f.barcode, ''))";
    for t in &toks {
        let ors: Vec<String> = expand_token(t)
            .into_iter()
            .map(|e| {
                p.push(format!("%{}%", e.replace('%', "")).into());
                format!("{hay} LIKE ?")
            })
            .collect();
        wh.push(format!("({})", ors.join(" OR ")));
    }
    let where_sql = if wh.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", wh.join(" AND "))
    };
    let uid = o.user_id.unwrap_or(-1);
    let sql = format!(
        "SELECT * FROM (SELECT {cols}, {ncols},
            (SELECT COUNT(*) FROM entries e WHERE e.food_id = f.id) AS uses,
            (SELECT COUNT(*) FROM entries e WHERE e.food_id = f.id AND e.user_id = {uid}) AS my_uses
         FROM foods f {where_sql}) ORDER BY my_uses DESC, uses DESC, id DESC LIMIT 400",
        cols = FOOD_COLS
            .split(", ")
            .map(|c| format!("f.{c}"))
            .collect::<Vec<_>>()
            .join(", "),
        ncols = nutrients::columns("f."),
    );
    let mut st = c.prepare(&sql)?;
    let rows = st.query_map(params_from_iter(p), |r| {
        let f = from_row(r)?;
        let uses: i64 = r.get(N_OFFSET + CATALOG.len())?;
        let my: i64 = r.get(N_OFFSET + CATALOG.len() + 1)?;
        Ok((f, uses, my))
    })?;
    let q = toks.join(" ");
    let mut scored = vec![];
    for row in rows {
        let (f, uses, my) = row?;
        let name = f.name.to_lowercase();
        let mut score = (my as f64 * 2.0 + uses as f64).min(40.0);
        if !q.is_empty() {
            if name == q {
                score += 100.0;
            } else if name.starts_with(&q) {
                score += 50.0;
            } else if name.contains(&q) {
                score += 25.0;
            }
            for t in &toks {
                if name.split(|c: char| !c.is_alphanumeric()).any(|w| w == t) {
                    score += 8.0;
                } else if name.contains(t.as_str()) {
                    score += 4.0;
                }
            }
            score -= name.len() as f64 / 25.0;
        }
        scored.push((score, f, uses));
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = vec![];
    for (_, f, uses) in scored.into_iter().take(o.limit.clamp(1, 100)) {
        out.push(SearchHit {
            food: finish(c, f)?,
            uses,
        });
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize)]
pub struct RecentFood {
    pub food: Food,
    pub uses: i64,
    pub last_date: String,
    pub last_quantity: f64,
    pub last_unit: String,
    pub last_serving_id: Option<i64>,
    pub last_meal: String,
    /// How many times this user logged the food at each meal (for "what do I usually have for dinner").
    pub meal_uses: std::collections::BTreeMap<String, i64>,
}

/// Foods this user logged recently, newest first, with the last quantity used so they can be re-logged
/// in one tap.
pub fn recent(c: &Connection, user_id: i64, limit: usize) -> AppResult<Vec<RecentFood>> {
    let mut st = c.prepare(
        "SELECT e.food_id, COUNT(*) AS uses, MAX(e.date || ' ' || e.created_at) AS last
         FROM entries e JOIN foods f ON f.id = e.food_id
         WHERE e.user_id = ? AND f.archived = 0 GROUP BY e.food_id ORDER BY last DESC LIMIT ?",
    )?;
    let rows: Vec<(i64, i64)> = st
        .query_map(params![user_id, limit as i64], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .collect::<Result<_, _>>()?;
    let mut out = vec![];
    for (fid, uses) in rows {
        let (date, qty, unit, sid, meal): (String, f64, String, Option<i64>, String) = c.query_row(
            "SELECT date, quantity, unit, serving_id, meal FROM entries WHERE user_id = ? AND food_id = ?
             ORDER BY date DESC, created_at DESC, id DESC LIMIT 1",
            params![user_id, fid],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
        let meal_uses = {
            let mut st = c.prepare_cached("SELECT meal, COUNT(*) FROM entries WHERE user_id = ? AND food_id = ? GROUP BY meal")?;
            let rows = st.query_map(params![user_id, fid], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })?;
            rows.collect::<Result<_, _>>()?
        };
        out.push(RecentFood {
            food: get(c, fid)?,
            uses,
            last_date: date,
            last_quantity: qty,
            last_unit: unit,
            last_serving_id: sid,
            last_meal: meal,
            meal_uses,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn food(basis_unit: &str, basis_amount: f64, servings: Vec<(i64, &str, f64, bool)>) -> Food {
        Food {
            id: 1,
            name: "Test".into(),
            brand: None,
            kind: "food".into(),
            basis_amount,
            basis_unit: basis_unit.into(),
            barcode: None,
            source: "manual".into(),
            source_id: None,
            aliases: None,
            notes: None,
            recipe_yield: None,
            archived: false,
            created_by: None,
            created_at: String::new(),
            updated_at: String::new(),
            nutrients: Nutrients::empty(),
            servings: servings
                .into_iter()
                .map(|(id, l, a, d)| Serving {
                    id,
                    label: l.into(),
                    amount: a,
                    is_default: d,
                })
                .collect(),
            ingredients: None,
            units: units_for(basis_unit),
            per_serving: None,
        }
    }

    #[test]
    fn resolve_units_and_servings() {
        let milk = food(
            "ml",
            100.0,
            vec![
                (7, "cup", 240.0, true),
                (8, "bottle (591 ml)", 591.0, false),
            ],
        );
        assert_eq!(
            resolve_amount(&milk, 2.0, None, None).unwrap().amount,
            480.0
        );
        assert_eq!(
            resolve_amount(&milk, 1.0, Some("Cups"), None)
                .unwrap()
                .amount,
            240.0,
            "serving label wins"
        );
        assert_eq!(
            resolve_amount(&milk, 1.0, Some("bottle"), None)
                .unwrap()
                .serving_id,
            Some(8)
        );
        assert_eq!(
            resolve_amount(&milk, 250.0, Some("ml"), None)
                .unwrap()
                .amount,
            250.0
        );
        assert_eq!(
            resolve_amount(&milk, 1.0, Some("l"), None).unwrap().amount,
            1000.0
        );
        assert_eq!(
            resolve_amount(&milk, 1.0, Some("serving"), None)
                .unwrap()
                .amount,
            240.0
        );
        assert_eq!(
            resolve_amount(&milk, 0.5, None, Some(8)).unwrap().amount,
            295.5
        );
        assert!(
            resolve_amount(&milk, 1.0, Some("g"), None).is_err(),
            "mass on a volume food"
        );
        assert!(resolve_amount(&milk, 0.0, None, None).is_err());
        assert!(resolve_amount(&milk, 1.0, None, Some(99)).is_err());

        let oats = food("g", 100.0, vec![]);
        assert_eq!(
            resolve_amount(&oats, 40.0, None, None).unwrap().amount,
            40.0
        );
        let r = resolve_amount(&oats, 2.0, Some("oz"), None).unwrap();
        assert_eq!(r.unit, "oz");
        assert!((r.amount - 56.699).abs() < 0.01);
        let err = resolve_amount(&oats, 1.0, Some("serving"), None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("no servings"), "{err}");

        let burrito = food("serving", 1.0, vec![]);
        assert_eq!(
            resolve_amount(&burrito, 0.5, Some("serving"), None)
                .unwrap()
                .amount,
            0.5
        );
        assert_eq!(
            resolve_amount(&burrito, 2.0, None, None).unwrap().amount,
            2.0
        );
    }

    #[test]
    fn basis_normalization() {
        assert_eq!(normalize_basis(None, None).unwrap(), (100.0, "g".into()));
        assert_eq!(
            normalize_basis(Some(1.0), Some("serving")).unwrap(),
            (1.0, "serving".into())
        );
        let (a, u) = normalize_basis(Some(3.0), Some("oz")).unwrap();
        assert_eq!(u, "g");
        assert!((a - 85.05).abs() < 0.01);
        assert_eq!(normalize_basis(Some(1.0), Some("cup")).unwrap().1, "ml");
        assert!(normalize_basis(Some(0.0), Some("g")).is_err());
        assert!(normalize_basis(Some(1.0), Some("bowl")).is_err());
    }

    #[test]
    fn tokenizing() {
        assert_eq!(tokens("Costco  milk, 2%"), vec!["costco", "milk", "2%"]);
        assert_eq!(expand_token("costco"), vec!["costco", "kirkland"]);
    }
}
