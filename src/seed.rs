//! Realistic demo data: two users, a shared food library with a recipe, and N days of diaries,
//! weights and goals. Used for screenshots, bug bashes and the Trends tests.

use crate::error::AppResult;
use crate::nutrients::Nutrients;
use crate::service::diary::{self, EntryInput, ExerciseInput};
use crate::service::foods::{self, FoodInput, IngredientInput, ServingInput};
use crate::service::goals::{self, GoalInput};
use crate::service::users::{self, UserInput};
use crate::service::weights::{self, WeightInput};
use crate::util::fmt_date;
use chrono::NaiveDate;
use rusqlite::Connection;
use serde_json::json;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f64) / ((1u64 << 31) as f64)
    }
    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next()
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[((self.next() * v.len() as f64) as usize).min(v.len() - 1)]
    }
}

fn food(
    c: &mut Connection,
    name: &str,
    brand: Option<&str>,
    basis: (f64, &str),
    n: serde_json::Value,
    servings: &[(&str, f64)],
    aliases: Option<&str>,
) -> AppResult<i64> {
    let inp = FoodInput {
        name: name.into(),
        brand: brand.map(String::from),
        basis_amount: Some(basis.0),
        basis_unit: Some(basis.1.into()),
        nutrients: Nutrients::from_json(&n).expect("valid demo nutrients"),
        servings: Some(
            servings
                .iter()
                .enumerate()
                .map(|(i, (l, a))| ServingInput {
                    id: None,
                    label: l.to_string(),
                    amount: *a,
                    unit: None,
                    is_default: i == 0,
                })
                .collect(),
        ),
        aliases: aliases.map(String::from),
        source: Some("manual".into()),
        ..Default::default()
    };
    Ok(foods::create(c, inp, None)?.food.id)
}

pub fn seed_demo(c: &mut Connection, days: i64, today: NaiveDate) -> AppResult<()> {
    let alex = users::create(
        c,
        UserInput {
            name: Some("Alex".into()),
            sex: Some(Some("male".into())),
            birth_date: Some(Some("1990-05-01".into())),
            height_cm: Some(Some(180.0)),
            activity_level: Some(Some(1.375)),
            goal_weight_kg: Some(Some(78.0)),
            ..Default::default()
        },
    )?;
    let sam = users::create(
        c,
        UserInput {
            name: Some("Sam".into()),
            sex: Some(Some("female".into())),
            birth_date: Some(Some("1992-09-14".into())),
            height_cm: Some(Some(165.0)),
            activity_level: Some(Some(1.55)),
            weight_unit: Some("lb".into()),
            goal_weight_kg: Some(Some(60.0)),
            ..Default::default()
        },
    )?;

    let oats = food(
        c,
        "Rolled oats",
        None,
        (100.0, "g"),
        json!({"calories": 379, "protein": 13.2, "carbs": 67.7, "fat": 6.5, "fiber": 10.1, "sugar": 1.0, "sodium": 6, "iron": 4.3, "magnesium": 138, "zinc": 3.6, "phosphorus": 410, "potassium": 362, "thiamin": 0.46, "saturated_fat": 1.1}),
        &[("1/2 cup", 40.0)],
        Some("oatmeal"),
    )?;
    let milk = food(
        c,
        "Whole Milk",
        Some("Kirkland Signature"),
        (100.0, "ml"),
        json!({"calories": 62, "protein": 3.33, "carbs": 5, "fat": 3.33, "sugar": 5, "saturated_fat": 2.08, "cholesterol": 10, "sodium": 44, "calcium": 117, "potassium": 133, "vitamin_d": 1.25, "vitamin_b12": 0.45, "riboflavin": 0.17, "vitamin_a": 46}),
        &[("cup", 240.0)],
        Some("costco milk"),
    )?;
    let banana = food(
        c,
        "Banana",
        None,
        (100.0, "g"),
        json!({"calories": 89, "protein": 1.09, "carbs": 22.8, "fat": 0.33, "fiber": 2.6, "sugar": 12.2, "potassium": 358, "vitamin_c": 8.7, "vitamin_b6": 0.367, "magnesium": 27, "folate": 20, "manganese": 0.27}),
        &[("medium", 118.0), ("large", 136.0)],
        None,
    )?;
    let pb = food(
        c,
        "Peanut butter",
        None,
        (100.0, "g"),
        json!({"calories": 588, "protein": 25, "carbs": 20, "fat": 50, "fiber": 6, "sugar": 9, "sodium": 459, "saturated_fat": 10, "vitamin_e": 9, "niacin": 13, "magnesium": 154}),
        &[("tbsp", 16.0)],
        None,
    )?;
    let eggs = food(
        c,
        "Egg",
        None,
        (100.0, "g"),
        json!({"calories": 143, "protein": 12.6, "carbs": 0.72, "fat": 9.5, "saturated_fat": 3.1, "cholesterol": 372, "sodium": 142, "vitamin_d": 2.0, "vitamin_b12": 0.89, "choline": 294, "selenium": 30.7, "riboflavin": 0.46, "vitamin_a": 160}),
        &[("large egg", 50.0)],
        Some("eggs"),
    )?;
    let bread = food(
        c,
        "Whole wheat bread",
        None,
        (100.0, "g"),
        json!({"calories": 247, "protein": 13, "carbs": 41, "fat": 3.4, "fiber": 7, "sugar": 6, "sodium": 450, "iron": 2.5, "thiamin": 0.4, "folate": 42}),
        &[("slice", 32.0)],
        Some("toast"),
    )?;
    let coffee = food(
        c,
        "Coffee, brewed",
        None,
        (100.0, "ml"),
        json!({"calories": 1, "protein": 0.12, "fat": 0.02, "carbs": 0, "caffeine": 40, "potassium": 49}),
        &[("cup", 240.0)],
        None,
    )?;
    let yogurt = food(
        c,
        "Greek yogurt, nonfat",
        Some("Fage"),
        (100.0, "g"),
        json!({"calories": 59, "protein": 10.2, "carbs": 3.6, "fat": 0.4, "sugar": 3.2, "calcium": 110, "vitamin_b12": 0.75, "sodium": 36, "potassium": 141}),
        &[("container", 170.0)],
        None,
    )?;
    let almonds = food(
        c,
        "Almonds",
        None,
        (100.0, "g"),
        json!({"calories": 579, "protein": 21.2, "carbs": 21.6, "fat": 49.9, "fiber": 12.5, "sugar": 4.4, "vitamin_e": 25.6, "magnesium": 270, "calcium": 269, "saturated_fat": 3.8, "riboflavin": 1.1}),
        &[("oz (23 almonds)", 28.0)],
        None,
    )?;
    let chicken = food(
        c,
        "Chicken breast, cooked",
        None,
        (100.0, "g"),
        json!({"calories": 165, "protein": 31, "carbs": 0, "fat": 3.6, "saturated_fat": 1.0, "cholesterol": 85, "sodium": 74, "potassium": 256, "phosphorus": 228, "niacin": 13.7, "vitamin_b6": 0.6, "selenium": 27.6}),
        &[],
        None,
    )?;
    let rice = food(
        c,
        "White rice, cooked",
        None,
        (100.0, "g"),
        json!({"calories": 130, "protein": 2.7, "carbs": 28.2, "fat": 0.3, "fiber": 0.4, "sodium": 1, "iron": 1.2, "folate": 58}),
        &[("cup", 158.0)],
        None,
    )?;
    let broccoli = food(
        c,
        "Broccoli",
        None,
        (100.0, "g"),
        json!({"calories": 34, "protein": 2.8, "carbs": 6.6, "fat": 0.4, "fiber": 2.6, "sugar": 1.7, "vitamin_c": 89.2, "vitamin_k": 101.6, "folate": 63, "potassium": 316, "vitamin_a": 31}),
        &[("cup chopped", 91.0)],
        None,
    )?;
    let salmon = food(
        c,
        "Salmon, Atlantic, cooked",
        None,
        (100.0, "g"),
        json!({"calories": 206, "protein": 22.1, "carbs": 0, "fat": 12.4, "saturated_fat": 2.5, "cholesterol": 63, "sodium": 61, "vitamin_d": 13.1, "vitamin_b12": 2.8, "selenium": 41.4, "niacin": 8.0, "potassium": 384}),
        &[("fillet", 154.0)],
        None,
    )?;
    let pasta = food(
        c,
        "Pasta, cooked",
        None,
        (100.0, "g"),
        json!({"calories": 158, "protein": 5.8, "carbs": 30.9, "fat": 0.9, "fiber": 1.8, "sodium": 1, "iron": 1.3, "selenium": 26}),
        &[("cup", 140.0)],
        None,
    )?;
    let spinach = food(
        c,
        "Spinach, raw",
        None,
        (100.0, "g"),
        json!({"calories": 23, "protein": 2.9, "carbs": 3.6, "fat": 0.4, "fiber": 2.2, "vitamin_a": 469, "vitamin_k": 483, "folate": 194, "iron": 2.7, "magnesium": 79, "vitamin_c": 28, "potassium": 558}),
        &[("cup", 30.0)],
        None,
    )?;
    let olive = food(
        c,
        "Olive oil",
        None,
        (100.0, "g"),
        json!({"calories": 884, "fat": 100, "protein": 0, "carbs": 0, "saturated_fat": 13.8, "monounsaturated_fat": 73, "polyunsaturated_fat": 10.5, "vitamin_e": 14.4, "vitamin_k": 60}),
        &[("tbsp", 13.5)],
        None,
    )?;
    let whey = food(
        c,
        "Whey protein",
        Some("Optimum Nutrition"),
        (100.0, "g"),
        json!({"calories": 387, "protein": 77, "carbs": 9.7, "fat": 4.8, "sugar": 3.2, "sodium": 419, "calcium": 419, "cholesterol": 97}),
        &[("scoop", 31.0)],
        Some("protein powder"),
    )?;
    let beans = food(
        c,
        "Black beans, cooked",
        None,
        (100.0, "g"),
        json!({"calories": 132, "protein": 8.9, "carbs": 23.7, "fat": 0.5, "fiber": 8.7, "folate": 149, "iron": 2.1, "magnesium": 70, "potassium": 355}),
        &[("cup", 172.0)],
        None,
    )?;
    let apple = food(
        c,
        "Apple",
        None,
        (100.0, "g"),
        json!({"calories": 52, "protein": 0.26, "carbs": 13.8, "fat": 0.17, "fiber": 2.4, "sugar": 10.4, "vitamin_c": 4.6, "potassium": 107}),
        &[("medium", 182.0)],
        None,
    )?;
    let bowl = food(
        c,
        "Burrito bowl",
        Some("Chipotle"),
        (1.0, "serving"),
        json!({"calories": 755, "protein": 46, "carbs": 76, "fat": 28, "fiber": 15, "sodium": 1960, "saturated_fat": 9}),
        &[],
        None,
    )?;

    let morning_oats = foods::create(
        c,
        FoodInput {
            name: "Morning oats".into(),
            kind: Some("recipe".into()),
            aliases: Some("oatmeal breakfast".into()),
            notes: Some("Oats cooked in milk with banana and peanut butter".into()),
            ingredients: Some(vec![
                IngredientInput {
                    food_id: oats,
                    quantity: 50.0,
                    unit: Some("g".into()),
                    serving_id: None,
                },
                IngredientInput {
                    food_id: milk,
                    quantity: 1.0,
                    unit: Some("cup".into()),
                    serving_id: None,
                },
                IngredientInput {
                    food_id: banana,
                    quantity: 1.0,
                    unit: Some("medium".into()),
                    serving_id: None,
                },
                IngredientInput {
                    food_id: pb,
                    quantity: 1.0,
                    unit: Some("tbsp".into()),
                    serving_id: None,
                },
            ]),
            recipe_yield: Some(1.0),
            ..Default::default()
        },
        Some(alex.id),
    )?
    .food
    .id;

    let start = today - chrono::Duration::days(days - 1);
    for (u, cal, protein, mid_cal) in [
        (&alex, 2300.0, 160.0, 2150.0),
        (&sam, 1750.0, 110.0, 1700.0),
    ] {
        let g = |c: &Connection, from: NaiveDate, cal: f64| {
            goals::set(
                c,
                u.id,
                GoalInput {
                    effective_from: Some(fmt_date(from)),
                    calories: Some(Some(cal)),
                    protein: Some(Some(protein)),
                    carbs: Some(Some(((cal - protein * 4.0) * 0.55 / 4.0).round())),
                    fat: Some(Some(((cal - protein * 4.0) * 0.45 / 9.0).round())),
                    fiber: Some(Some(30.0)),
                    exercise_adds_to_budget: Some(false),
                    micros: None,
                },
                today,
            )
        };
        g(c, start, cal)?;
        if days > 30 {
            g(c, today - chrono::Duration::days(20), mid_cal)?;
        }
    }

    type Item = (i64, f64, &'static str);
    let breakfasts: Vec<Vec<Item>> = vec![
        vec![(morning_oats, 1.0, "serving"), (coffee, 1.0, "cup")],
        vec![
            (eggs, 2.0, "large egg"),
            (bread, 2.0, "slice"),
            (coffee, 1.0, "cup"),
        ],
        vec![
            (yogurt, 1.0, "container"),
            (banana, 1.0, "medium"),
            (almonds, 0.5, "oz (23 almonds)"),
        ],
    ];
    let lunches: Vec<Vec<Item>> = vec![
        vec![
            (chicken, 150.0, "g"),
            (rice, 1.0, "cup"),
            (broccoli, 1.0, "cup chopped"),
        ],
        vec![
            (beans, 1.0, "cup"),
            (rice, 1.0, "cup"),
            (spinach, 2.0, "cup"),
            (olive, 0.5, "tbsp"),
        ],
        vec![(bowl, 1.0, "serving")],
    ];
    let dinners: Vec<Vec<Item>> = vec![
        vec![
            (salmon, 1.0, "fillet"),
            (pasta, 1.5, "cup"),
            (spinach, 2.0, "cup"),
            (olive, 1.0, "tbsp"),
        ],
        vec![
            (chicken, 180.0, "g"),
            (pasta, 1.5, "cup"),
            (broccoli, 1.0, "cup chopped"),
            (olive, 1.0, "tbsp"),
        ],
        vec![
            (eggs, 3.0, "large egg"),
            (bread, 2.0, "slice"),
            (spinach, 1.0, "cup"),
            (apple, 1.0, "medium"),
        ],
    ];
    let snacks: Vec<Vec<Item>> = vec![
        vec![(apple, 1.0, "medium")],
        vec![(whey, 1.0, "scoop"), (milk, 1.0, "cup")],
        vec![(almonds, 1.0, "oz (23 almonds)")],
        vec![(whey, 1.0, "scoop"), (banana, 1.0, "medium")],
    ];

    let mut rng = Rng(42);
    for (u, scale, w0, w_slope) in [(&alex, 1.0, 84.0, -0.04), (&sam, 0.72, 63.5, -0.02)] {
        for i in 0..days {
            let day = start + chrono::Duration::days(i);
            let ds = fmt_date(day);
            let is_today = day == today;
            if rng.next() < 0.08 && !is_today {
                continue; // an unlogged day
            }
            let meals: Vec<(&str, &Vec<Item>)> = vec![
                ("breakfast", rng.pick(&breakfasts)),
                ("lunch", rng.pick(&lunches)),
                ("dinner", rng.pick(&dinners)),
                ("snacks", rng.pick(&snacks)),
            ];
            for (meal, items) in meals {
                if is_today && (meal == "dinner" || meal == "snacks") {
                    continue;
                }
                // A realistic time for each sitting, e.g. breakfast 7:15-8:45.
                let (lo, hi) = match meal {
                    "breakfast" => (7.25, 8.75),
                    "lunch" => (12.0, 13.5),
                    "dinner" => (18.75, 20.0),
                    _ => (15.0, 16.5),
                };
                let t = rng.range(lo, hi);
                let time = format!("{:02}:{:02}", t as u32, ((t.fract() * 60.0) as u32 / 5) * 5);
                for (fid, qty, unit) in items {
                    let q = (qty * scale * rng.range(0.85, 1.2) * 4.0).round() / 4.0;
                    diary::add_entry(
                        c,
                        u.id,
                        EntryInput {
                            date: Some(ds.clone()),
                            meal: Some(meal.into()),
                            food_id: Some(*fid),
                            quantity: Some(q.max(0.25)),
                            unit: Some(unit.to_string()),
                            source: Some("demo".into()),
                            time: Some(Some(time.clone())),
                            ..Default::default()
                        },
                        today,
                        chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
                    )?;
                }
            }
            if rng.next() < 0.3 {
                diary::add_exercise(
                    c,
                    u.id,
                    ExerciseInput {
                        date: Some(ds.clone()),
                        name: rng
                            .pick(&["Run", "Cycling", "Strength training", "Swim"])
                            .to_string(),
                        calories: (rng.range(200.0, 500.0) * scale).round(),
                        duration_min: Some(rng.range(30.0, 70.0).round()),
                    },
                    today,
                )?;
            }
            if rng.next() < 0.7 || i == days - 1 {
                let kg = w0 + w_slope * i as f64 + rng.range(-0.6, 0.6);
                let (v, unit) = if u.weight_unit == "lb" {
                    (kg / crate::units::KG_PER_LB, "lb")
                } else {
                    (kg, "kg")
                };
                weights::upsert(
                    c,
                    u.id,
                    &u.weight_unit,
                    WeightInput {
                        date: Some(ds.clone()),
                        weight: (v * 10.0).round() / 10.0,
                        unit: Some(unit.into()),
                        body_fat_pct: None,
                        note: None,
                    },
                    today,
                )?;
            }
        }
    }
    Ok(())
}
