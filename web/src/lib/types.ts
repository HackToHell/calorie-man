export type Nutrients = Record<string, number>;

export interface NutrientDef {
  key: string;
  label: string;
  unit: string;
  group: 'energy' | 'macros' | 'fats' | 'minerals' | 'vitamins' | 'other';
  dv: number | null;
  limit: boolean;
  aliases: string[];
}

export interface Catalog {
  nutrients: NutrientDef[];
  meals: string[];
  units: Record<string, string[]>;
  activity_levels: [number, string][];
}

export interface User {
  id: number;
  name: string;
  color: string;
  sex: 'male' | 'female' | null;
  birth_date: string | null;
  height_cm: number | null;
  activity_level: number | null;
  weight_unit: 'kg' | 'lb';
  timezone: string | null;
  goal_weight_kg: number | null;
  created_at: string;
}

export interface Serving {
  id: number;
  label: string;
  amount: number;
  is_default: boolean;
}

export interface Ingredient {
  id: number;
  food_id: number;
  name: string;
  brand: string | null;
  quantity: number;
  unit: string;
  serving_id: number | null;
  amount: number;
  basis_unit: string;
  nutrients: Nutrients;
}

export interface Food {
  id: number;
  name: string;
  brand: string | null;
  kind: 'food' | 'recipe';
  basis_amount: number;
  basis_unit: 'g' | 'ml' | 'serving';
  barcode: string | null;
  source: string;
  source_id: string | null;
  aliases: string | null;
  notes: string | null;
  recipe_yield: number | null;
  archived: boolean;
  created_by: number | null;
  created_at: string;
  updated_at: string;
  nutrients: Nutrients;
  servings: Serving[];
  ingredients?: Ingredient[];
  units: string[];
  per_serving: { label: string; amount: number; calories: number | null; protein: number | null; carbs: number | null; fat: number | null } | null;
  uses?: number;
}

export interface RecentFood {
  food: Food;
  uses: number;
  last_date: string;
  last_quantity: number;
  last_unit: string;
  last_serving_id: number | null;
  last_meal: string;
  meal_uses: Record<string, number>;
}

export interface Entry {
  id: number;
  user_id: number;
  date: string;
  meal: string;
  /** "HH:MM" local time it was eaten, if known. */
  time: string | null;
  food_id: number | null;
  name: string;
  brand: string | null;
  quantity: number;
  unit: string;
  serving_id: number | null;
  amount: number | null;
  note: string | null;
  source: string;
  created_at: string;
  nutrients: Nutrients;
  display_quantity: string;
}

export interface Exercise {
  id: number;
  user_id: number;
  date: string;
  name: string;
  calories: number;
  duration_min: number | null;
}

export interface Goal {
  id: number;
  user_id: number;
  effective_from: string;
  calories: number | null;
  protein: number | null;
  carbs: number | null;
  fat: number | null;
  fiber: number | null;
  exercise_adds_to_budget: boolean;
  micros: Record<string, number>;
}

export interface Target {
  key: string;
  target: number;
  limit: boolean;
  from: 'goal' | 'dv';
}

export interface Weight {
  id: number;
  user_id: number;
  date: string;
  weight_kg: number;
  body_fat_pct: number | null;
  note: string | null;
  trend_kg?: number;
}

export interface Diary {
  date: string;
  user_id: number;
  entries: Entry[];
  exercises: Exercise[];
  totals: Nutrients;
  meals: Record<string, Nutrients>;
  goal: Goal | null;
  targets: Target[];
  budget: {
    goal: number | null;
    exercise: number;
    exercise_adds_to_budget: boolean;
    budget: number | null;
    eaten: number;
    remaining: number | null;
  };
  week: {
    start: string;
    end: string;
    logged_days: number;
    balance: number;
    weekly_budget: number | null;
    eaten: number;
    weekly_remaining: number | null;
  };
  weight: Weight | null;
  coverage: Record<string, number>;
  streak: { on_budget: number; logged: number };
  timing: Timing;
}

export interface Occasion {
  meal: string;
  time: string;
  calories: number;
  protein: number;
  entries: number;
}

export interface Timing {
  occasions: Occasion[];
  /** Usual time per meal ("HH:MM"), from the previous four weeks. */
  usual: Record<string, string>;
  first: string | null;
  last: string | null;
  untimed: number;
}

export interface SummaryDay {
  date: string;
  logged: boolean;
  entries: number;
  nutrients: Nutrients;
  exercise: number;
  goal_calories: number | null;
  budget: number | null;
  protein_target: number | null;
  weight_kg: number | null;
  trend_kg: number | null;
  body_fat_pct: number | null;
}

export interface Stats {
  days: number;
  logged_days: number;
  avg_calories: number | null;
  avg_protein: number | null;
  avg_carbs: number | null;
  avg_fat: number | null;
  avg_fiber: number | null;
  avg_budget: number | null;
  budget_days: number;
  within_budget_days: number;
  balance: number;
  protein_target_days: number;
  protein_hit_days: number;
  protein_hit_rate: number | null;
  protein_streak: number;
  avg_protein_per_kg: number | null;
  protein_pct_kcal: number | null;
  carbs_pct_kcal: number | null;
  fat_pct_kcal: number | null;
  weight_start_kg: number | null;
  weight_end_kg: number | null;
  weight_change_kg: number | null;
  weight_rate_kg_per_week: number | null;
  weigh_ins: number;
  estimated_tdee: number | null;
  estimated_tdee_note: string | null;
}

export interface MicroAvg {
  key: string;
  label: string;
  unit: string;
  avg: number | null;
  target: number | null;
  limit: boolean;
  pct: number | null;
  days_with_data: number;
}

export interface Summary {
  user_id: number;
  from: string;
  to: string;
  days: SummaryDay[];
  stats: Stats;
  micros: MicroAvg[];
}

export interface Energy {
  age: number | null;
  sex: string | null;
  height_cm: number | null;
  weight_kg: number | null;
  activity_level: number | null;
  bmr: number | null;
  tdee: number | null;
  adaptive_tdee: number | null;
  adaptive_note: string | null;
  suggestions: { rate_kg_per_week: number; calories: number; warning: string | null }[];
  protein_suggestion_g: number | null;
  missing: string[];
  activity_levels: [number, string][];
}

export interface Candidate {
  source: 'usda' | 'off';
  source_id: string;
  name: string;
  brand: string | null;
  barcode: string | null;
  category: string | null;
  data_type: string | null;
  basis_amount: number;
  basis_unit: string;
  servings: { label: string; amount: number }[];
  nutrients: Nutrients;
  nutrient_count: number;
  in_library: number | null;
  url: string | null;
}

export interface Settings {
  usda_api_key_set: boolean;
  usda_api_key_hint: string | null;
  usda_key_source: string;
  offline: boolean;
}
