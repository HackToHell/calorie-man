use crate::error::{bad, AppResult};
use crate::nutrients::{round_to, Nutrients, CATALOG};
use crate::service::diary::{budget_for, daily_totals, exercise_totals};
use crate::service::goals::{self};
use crate::service::users::{self, User};
use crate::service::weights;
use crate::util::{fmt_date, parse_date};
use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::BTreeMap;

pub const KCAL_PER_KG: f64 = 7700.0;

#[derive(Debug, Serialize)]
pub struct Day {
    pub date: String,
    pub logged: bool,
    pub entries: i64,
    pub nutrients: Nutrients,
    pub exercise: f64,
    pub goal_calories: Option<f64>,
    pub budget: Option<f64>,
    pub protein_target: Option<f64>,
    pub weight_kg: Option<f64>,
    pub trend_kg: Option<f64>,
    pub body_fat_pct: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct MicroAvg {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
    pub avg: Option<f64>,
    pub target: Option<f64>,
    pub limit: bool,
    pub pct: Option<f64>,
    /// Logged days on which at least one entry reported this nutrient.
    pub days_with_data: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct Stats {
    /// Completed days in the range (today is excluded while it is in progress).
    pub days: usize,
    pub logged_days: usize,
    pub avg_calories: Option<f64>,
    pub avg_protein: Option<f64>,
    pub avg_carbs: Option<f64>,
    pub avg_fat: Option<f64>,
    pub avg_fiber: Option<f64>,
    pub avg_budget: Option<f64>,
    pub budget_days: usize,
    pub within_budget_days: usize,
    /// Sum over logged days of (budget - eaten). Positive = under budget.
    pub balance: f64,
    pub protein_target_days: usize,
    pub protein_hit_days: usize,
    pub protein_hit_rate: Option<f64>,
    /// Consecutive logged days, ending at the most recent one, that reached the protein target.
    pub protein_streak: usize,
    pub avg_protein_per_kg: Option<f64>,
    pub protein_pct_kcal: Option<f64>,
    pub carbs_pct_kcal: Option<f64>,
    pub fat_pct_kcal: Option<f64>,
    pub weight_start_kg: Option<f64>,
    pub weight_end_kg: Option<f64>,
    pub weight_change_kg: Option<f64>,
    pub weight_rate_kg_per_week: Option<f64>,
    pub weigh_ins: usize,
    /// Maintenance calories estimated from what was eaten and how the weight trend moved.
    pub estimated_tdee: Option<f64>,
    pub estimated_tdee_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub user_id: i64,
    /// Start of the data actually summarized: the requested start, or the person's first log or
    /// weigh-in if that is later (so a 1-year view of 3 weeks of data is not mostly empty).
    pub from: String,
    pub requested_from: String,
    pub to: String,
    pub days: Vec<Day>,
    pub stats: Stats,
    pub micros: Vec<MicroAvg>,
}

fn avg(sum: f64, n: usize) -> Option<f64> {
    (n > 0).then(|| round_to(sum / n as f64, 1))
}

pub fn summary(
    c: &Connection,
    user: &User,
    from: NaiveDate,
    to: NaiveDate,
    today: NaiveDate,
) -> AppResult<Summary> {
    if to < from {
        return Err(bad("'to' must not be before 'from'"));
    }
    if (to - from).num_days() + 1 > 1100 {
        return Err(bad("the range is limited to 3 years"));
    }
    let requested_from = fmt_date(from);
    let first: Option<String> = c.query_row(
        "SELECT MIN(d) FROM (SELECT MIN(date) AS d FROM entries WHERE user_id = ?1
                             UNION ALL SELECT MIN(date) FROM weights WHERE user_id = ?1)",
        [user.id],
        |r| r.get(0),
    )?;
    let from = match first.as_deref().map(crate::util::parse_date).transpose()? {
        Some(f) if f > from && f <= to => f,
        _ => from,
    };
    let span = (to - from).num_days() + 1;
    let (fs, ts) = (fmt_date(from), fmt_date(to));
    let totals = daily_totals(c, user.id, &fs, &ts)?;
    let ex = exercise_totals(c, user.id, &fs, &ts)?;
    let goals_all = goals::list(c, user.id)?;
    let all_w = weights::list(c, user.id, None, Some(&ts))?;
    let trend = weights::trend(&all_w);
    let raw: BTreeMap<&str, &weights::Weight> =
        all_w.iter().map(|w| (w.date.as_str(), w)).collect();

    let mut days = Vec::with_capacity(span as usize);
    let mut ti = 0usize;
    let mut cur_trend: Option<f64> = None;
    for i in 0..span {
        let d = from + chrono::Duration::days(i);
        let ds = fmt_date(d);
        while ti < trend.len() && trend[ti].0 <= ds {
            cur_trend = Some(trend[ti].2);
            ti += 1;
        }
        let g = goals::pick(&goals_all, &ds);
        let exc = ex.get(&ds).copied().unwrap_or(0.0);
        let (n, cnt) = totals.get(&ds).cloned().unwrap_or((Nutrients::empty(), 0));
        let w = raw.get(ds.as_str());
        days.push(Day {
            date: ds.clone(),
            logged: cnt > 0,
            entries: cnt,
            nutrients: n,
            exercise: exc,
            goal_calories: g.and_then(|g| g.calories),
            budget: budget_for(g, exc),
            protein_target: g.and_then(|g| g.protein),
            weight_kg: w.map(|w| w.weight_kg),
            trend_kg: cur_trend,
            body_fat_pct: w.and_then(|w| w.body_fat_pct),
        });
    }

    // Today is still in progress (and later days have not happened), so only earlier days count for
    // averages, hit rates and the balance.
    let today_s = fmt_date(today);
    let logged: Vec<&Day> = days
        .iter()
        .filter(|d| d.logged && d.date < today_s)
        .collect();
    let complete_days = days.iter().filter(|d| d.date < today_s).count();
    let mut s = Stats {
        days: complete_days,
        logged_days: logged.len(),
        ..Default::default()
    };
    let sum = |k: &str| logged.iter().fold(0.0, |a, d| a + d.nutrients.val(k));
    s.avg_calories = avg(sum("calories"), logged.len());
    s.avg_protein = avg(sum("protein"), logged.len());
    s.avg_carbs = avg(sum("carbs"), logged.len());
    s.avg_fat = avg(sum("fat"), logged.len());
    s.avg_fiber = avg(sum("fiber"), logged.len());
    let mut budget_sum = 0.0;
    for d in &logged {
        if let Some(b) = d.budget {
            s.budget_days += 1;
            budget_sum += b;
            s.balance += b - d.nutrients.val("calories");
            if d.nutrients.val("calories") <= b {
                s.within_budget_days += 1;
            }
        }
        if let Some(t) = d.protein_target {
            s.protein_target_days += 1;
            if d.nutrients.val("protein") + 0.5 >= t {
                s.protein_hit_days += 1;
            }
        }
    }
    s.balance = round_to(s.balance, 0);
    s.avg_budget = avg(budget_sum, s.budget_days);
    if s.protein_target_days > 0 {
        s.protein_hit_rate = Some(round_to(
            s.protein_hit_days as f64 / s.protein_target_days as f64,
            3,
        ));
    }
    // Streak: consecutive completed days (ending yesterday or at the range end) that reached the target.
    for d in days.iter().rev().filter(|d| d.date < today_s) {
        if !d.logged {
            break;
        }
        match d
            .protein_target
            .map(|t| d.nutrients.val("protein") + 0.5 >= t)
        {
            Some(true) => s.protein_streak += 1,
            _ => break,
        }
    }
    let kcal_p = sum("protein") * 4.0;
    let kcal_c = sum("carbs") * 4.0;
    let kcal_f = sum("fat") * 9.0;
    let kcal_m = kcal_p + kcal_c + kcal_f;
    if kcal_m > 0.0 {
        s.protein_pct_kcal = Some(round_to(kcal_p / kcal_m * 100.0, 1));
        s.carbs_pct_kcal = Some(round_to(kcal_c / kcal_m * 100.0, 1));
        s.fat_pct_kcal = Some(round_to(kcal_f / kcal_m * 100.0, 1));
    }
    // Weight trend over the range.
    let in_range: Vec<&Day> = days.iter().filter(|d| d.trend_kg.is_some()).collect();
    s.weigh_ins = days.iter().filter(|d| d.weight_kg.is_some()).count();
    let first_w = days.iter().find(|d| d.weight_kg.is_some());
    let last_w = days.iter().rev().find(|d| d.weight_kg.is_some());
    if let (Some(a), Some(b)) = (first_w, last_w) {
        let (ta, tb) = (a.trend_kg.unwrap(), b.trend_kg.unwrap());
        s.weight_start_kg = Some(round_to(ta, 2));
        s.weight_end_kg = Some(round_to(tb, 2));
        s.weight_change_kg = Some(round_to(tb - ta, 2));
        let span_days = (parse_date(&b.date)? - parse_date(&a.date)?).num_days();
        if span_days >= 7 {
            s.weight_rate_kg_per_week = Some(round_to((tb - ta) / span_days as f64 * 7.0, 3));
        }
        if let (Some(p), Some(w)) = (s.avg_protein, in_range.last().and_then(|d| d.trend_kg)) {
            s.avg_protein_per_kg = Some(round_to(p / w, 2));
        }
        // Energy balance: eaten - change in stored energy.
        if s.logged_days >= 10 && span_days >= 14 {
            let tdee = s.avg_calories.unwrap_or(0.0) - (tb - ta) * KCAL_PER_KG / span_days as f64;
            s.estimated_tdee = Some((tdee / 10.0).round() * 10.0);
            let coverage = s.logged_days as f64 / s.days.max(1) as f64;
            s.estimated_tdee_note = Some(if coverage < 0.8 {
                format!(
                    "Low confidence: only {} of {} days are logged; unlogged days are assumed to look like logged ones.",
                    s.logged_days, s.days
                )
            } else if span_days < 28 {
                "Rough estimate: weight trends need about 4 weeks to settle.".into()
            } else {
                format!(
                    "Based on {} logged days and {} weigh-ins.",
                    s.logged_days, s.weigh_ins
                )
            });
        }
    } else if let Some(p) = s.avg_protein {
        if let Some(w) = weights::latest(c, user.id, &ts)? {
            s.avg_protein_per_kg = Some(round_to(p / w.weight_kg, 2));
        }
    }

    // Micronutrient averages against the goal in effect at the end of the range.
    let goal_end = goals::pick(&goals_all, &ts);
    let targets = goals::targets(goal_end);
    let mut micros = vec![];
    for (i, n) in CATALOG.iter().enumerate() {
        if matches!(n.key, "calories") {
            continue;
        }
        let with_data = logged.iter().filter(|d| d.nutrients.0[i].is_some()).count();
        let total: f64 = logged
            .iter()
            .filter_map(|d| d.nutrients.0[i])
            .fold(0.0, |a, b| a + b);
        let a = (with_data > 0).then(|| round_to(total / logged.len() as f64, 3));
        let t = targets.iter().find(|t| t.key == n.key).map(|t| t.target);
        micros.push(MicroAvg {
            key: n.key,
            label: n.label,
            unit: n.unit,
            avg: a,
            target: t,
            limit: n.limit,
            pct: match (a, t) {
                (Some(a), Some(t)) if t > 0.0 => Some(round_to(a / t * 100.0, 1)),
                _ => None,
            },
            days_with_data: with_data,
        });
    }
    Ok(Summary {
        user_id: user.id,
        from: fs,
        requested_from,
        to: ts,
        days,
        stats: s,
        micros,
    })
}

// ---------------------------------------------------------------------------------------------------------
// Energy needs

pub const ACTIVITY_LEVELS: &[(f64, &str)] = &[
    (1.2, "Sedentary (desk job, little exercise)"),
    (1.375, "Light (exercise 1-3 days/week)"),
    (1.55, "Moderate (exercise 3-5 days/week)"),
    (1.725, "Very active (hard exercise 6-7 days/week)"),
    (1.9, "Athlete (physical job plus training)"),
];

/// Mifflin-St Jeor resting energy expenditure.
pub fn bmr(sex: &str, kg: f64, cm: f64, age: i32) -> f64 {
    let base = 10.0 * kg + 6.25 * cm - 5.0 * age as f64;
    if sex == "female" {
        base - 161.0
    } else {
        base + 5.0
    }
}

#[derive(Debug, Serialize)]
pub struct Suggestion {
    pub rate_kg_per_week: f64,
    pub calories: f64,
    pub warning: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Energy {
    pub age: Option<i32>,
    pub sex: Option<String>,
    pub height_cm: Option<f64>,
    pub weight_kg: Option<f64>,
    pub activity_level: Option<f64>,
    pub bmr: Option<f64>,
    pub tdee: Option<f64>,
    /// From the last 28 days of logs and weigh-ins, when there is enough data.
    pub adaptive_tdee: Option<f64>,
    pub adaptive_note: Option<String>,
    pub suggestions: Vec<Suggestion>,
    pub protein_suggestion_g: Option<f64>,
    pub missing: Vec<&'static str>,
    pub activity_levels: Vec<(f64, &'static str)>,
}

pub fn energy(c: &Connection, user: &User, today: NaiveDate) -> AppResult<Energy> {
    let ts = fmt_date(today);
    let w = weights::list(c, user.id, None, Some(&ts))?;
    let weight = weights::trend(&w).last().map(|t| round_to(t.2, 2));
    let age = users::age_on(user, today);
    let mut missing = vec![];
    if user.sex.is_none() {
        missing.push("sex");
    }
    if age.is_none() {
        missing.push("birth_date");
    }
    if user.height_cm.is_none() {
        missing.push("height_cm");
    }
    if weight.is_none() {
        missing.push("weight");
    }
    let b = match (&user.sex, weight, user.height_cm, age) {
        (Some(s), Some(kg), Some(cm), Some(a)) => Some(bmr(s, kg, cm, a).round()),
        _ => None,
    };
    let activity = user.activity_level;
    let tdee = b.map(|b| (b * activity.unwrap_or(1.2)).round());
    let sm = summary(c, user, today - chrono::Duration::days(27), today, today)?;
    let adaptive = sm.stats.estimated_tdee;
    let basis = adaptive.or(tdee);
    let floor = if user.sex.as_deref() == Some("female") {
        1200.0
    } else {
        1500.0
    };
    let suggestions = match basis {
        Some(t) => [-1.0, -0.75, -0.5, -0.25, 0.0, 0.25, 0.5]
            .iter()
            .map(|r| {
                let cal = ((t + r * KCAL_PER_KG / 7.0) / 10.0).round() * 10.0;
                Suggestion {
                    rate_kg_per_week: *r,
                    calories: cal,
                    warning: (cal < floor)
                        .then(|| format!("below {floor:.0} kcal; consider a slower rate")),
                }
            })
            .collect(),
        None => vec![],
    };
    let ref_weight = user.goal_weight_kg.or(weight);
    Ok(Energy {
        age,
        sex: user.sex.clone(),
        height_cm: user.height_cm,
        weight_kg: weight,
        activity_level: activity,
        bmr: b,
        tdee,
        adaptive_tdee: adaptive,
        adaptive_note: sm.stats.estimated_tdee_note,
        suggestions,
        protein_suggestion_g: ref_weight.map(|w| (w * 1.6).round()),
        missing,
        activity_levels: ACTIVITY_LEVELS.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mifflin() {
        assert_eq!(bmr("male", 80.0, 180.0, 30).round(), 1780.0);
        assert_eq!(bmr("female", 60.0, 165.0, 30).round(), 1320.0);
    }
}
