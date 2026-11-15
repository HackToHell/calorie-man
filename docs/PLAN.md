# Eat: design plan

A calorie, macro and micronutrient tracker for a home network. One Rust binary serves the web UI, a JSON
API and an MCP endpoint. Data lives in a single SQLite file.

## Goals

| Need | Decision |
|---|---|
| Runs on the LAN, a couple of users, no auth | Header user switcher, remembered per device (localStorage). No passwords. Bind `0.0.0.0` by default. Do not expose to the internet. |
| Shared food library | `foods`, `food_servings` and `recipe_items` are global. Both users log the same "Morning oats". Diaries, goals, weights and exercise are per user. |
| All macros and vitamins | One nutrient catalog (39 nutrients: energy, macros, fat breakdown, sugars, 11 minerals, 14 vitamins, choline, caffeine, alcohol) drives the DB columns, API validation, MCP schema and UI forms. NULL means "unknown", which is different from 0 and is reported as coverage. |
| Tracking over time | Trends page: calories vs budget, protein vs target, weight with a smoothed trend, macro split, cumulative balance, estimated TDEE from intake and the weight trend, micronutrient averages vs daily value. |
| Calorie budgeting | Effective-dated goals (a goal change does not rewrite history). Daily budget = goal (+ exercise when enabled). Weekly bank: budget minus eaten so far this week. Goal calculator (Mifflin-St Jeor + activity + desired rate). |
| Protein tracking | Protein target per day, hit/miss per day, hit rate, streak, g/kg of body weight, share of calories. |
| Weight DB | One weigh-in per user per day (re-entering replaces it), optional body fat. Stored in kg, shown in the user's unit (kg/lb). EMA trend (alpha 0.1). Goal weight and rate per week. |
| AI food entry | MCP over Streamable HTTP at `/mcp` (stateless, JSON responses) plus `calorie-man mcp-stdio` bridge for stdio-only clients. Tools cover search (local and online), import, flexible create (any nutrient subset, named servings), recipes, logging, quick add, diary, weights, goals and summaries. |
| Product lookup | USDA FoodData Central (free key, `DEMO_KEY` fallback; branded foods include Kirkland/Costco with UPCs, generic foods have full vitamin profiles) and Open Food Facts (no key; barcode lookups and international products). "costco" is expanded to "kirkland" when searching. Results are cached in SQLite for 7 days. When nothing matches, the AI creates the food with `create_food`. |
| Fast UI | Svelte 5 + Vite + TypeScript, hand-written SVG charts (no chart library), built into `web/dist` and embedded in the binary. Mobile first, light/dark. |

## UI design

The web UI follows `docs/design/calorie-man-terrace-2a.html` (the "Terrace" design): stepped corners (`--terrace`
clip-path, step size `--t`), the plum, cream and pastel palette (peach = calories, lilac = protein,
butter = carbs, sea = fat, sky = fibre), Federo for display numbers and Outfit for text, both bundled in
`web/public/fonts`. Light and dark themes follow the system or the Profile setting.
- Desktop: a sidebar (Today, Log food, Food bank, Recipes, Trends, Weight, Goals, Profile) and a dashboard
  grid.
- Phone: a bottom bar (Today, Recipes, +, Trends, Profile) and the next-meal card.

## Architecture

```
src/
  main.rs         CLI: serve | mcp-stdio | seed-demo
  config.rs       bind address, db path, external API base URLs (overridable for tests)
  db.rs           pool, schema, migrations generated from the nutrient catalog
  nutrients.rs    the catalog (key, label, unit, group, daily value, target or limit, USDA/OFF mapping)
  units.rs        g/kg/oz/lb, ml/l/fl oz/cup/tbsp/tsp, servings
  models.rs       API types
  service/        all business logic, shared by REST and MCP
    users.rs foods.rs recipes.rs diary.rs weights.rs goals.rs summary.rs external.rs
  api.rs          REST routes (/api/...)
  mcp.rs          JSON-RPC 2.0 / MCP tools
  web.rs          embedded static files with SPA fallback
web/              Svelte app, Playwright tests
docs/             plan, MCP guide, bug bash workflow
```

The REST handlers and the MCP tools both call the `service` layer, so the two interfaces behave the same.

## Data model (SQLite, WAL, foreign keys on)

- `users(id, name unique nocase, color, sex, birth_date, height_cm, activity_level, weight_unit, timezone, goal_weight_kg, created_at)`
- `goals(id, user_id, effective_from, calories, protein, carbs, fat, fiber, exercise_adds_to_budget, micros JSON)`;
  the row with the latest `effective_from <= day` applies to that day.
- `foods(id, name, brand, kind food|recipe, basis_amount, basis_unit g|ml|serving, barcode, source manual|ai|usda|off|recipe, source_id, aliases, notes, recipe_yield, archived, created_by, created_at, updated_at, n_<nutrient>...)`.
  Nutrients are per `basis_amount basis_unit`: 100 g for most products, 1 serving for a restaurant item,
  or the whole pot for a recipe.
- `food_servings(id, food_id, label, amount in basis units, is_default)`.
- `recipe_items(id, recipe_id, food_id, quantity, unit, serving_id)`. A recipe's nutrients are the sum of its
  ingredients. With a cooked weight the basis is grams (log 250 g of chili), otherwise servings
  (basis = yield). Editing an ingredient recomputes every recipe that uses it. Cycles are rejected.
- `entries(id, user_id, date, meal, eaten_at "HH:MM" nullable, food_id nullable, name, brand, quantity, unit, serving_id, amount, note, source, created_at, n_<nutrient>...)`.
  The eaten-at time drives the meal-timing chart. Entries of one meal within an hour form a sitting. A
  meal's usual time is the median of its first time over the previous 28 days (with 3+ days of data).
  The eating window runs from the first time to now (or to the last time on past days). The v2 migration
  back-filled times from `created_at` for entries logged on their own day.
  Each entry keeps a snapshot of its nutrients so history survives food edits and deletes. Editing a food
  updates past entries by default (fixing a typo should fix history); the API can skip that.
  Quick-add entries have no food.
- `exercises(id, user_id, date, name, calories, duration_min)`.
- `weights(id, user_id, date, weight_kg, body_fat_pct, note)`, unique per user and day.
- `settings(key, value)` (USDA key), `external_cache(url, body, fetched_at)`.

Weeks run Monday to Sunday (weekly bank, dashboard week chart, calendar). Dates are local calendar days (`YYYY-MM-DD`). The browser sends its own date. MCP calls without a date use
"today" in the user's time zone (default: the server's).

Deleting a food archives it (hidden from search, entries and recipes keep working).

## Units

A quantity is `quantity` + `unit`. The unit can be the basis unit, a compatible mass or volume unit
(g, kg, oz, lb / ml, l, fl oz, cup, tbsp, tsp), `serving` (the default serving), or a named serving by id or
label ("cup", "1 bottle", "slice"). Mass and volume never convert into each other, because no density is
stored; add a named serving to bridge them.

## REST API (JSON)

```
GET  /api/health                 GET  /api/nutrients
GET  /api/users                  POST /api/users            GET|PUT|DELETE /api/users/:id
GET  /api/users/:id/goals        POST /api/users/:id/goals  DELETE /api/goals/:id
GET  /api/users/:id/tdee         goal calculator inputs and estimates
GET  /api/foods?q=&limit=&kind=  POST /api/foods            GET|PUT|DELETE /api/foods/:id
GET  /api/users/:id/recent-foods
GET  /api/external/search?q=&source=usda|off|all
GET  /api/external/barcode/:code POST /api/external/import {source, id}
GET  /api/users/:id/diary?date=
POST /api/users/:id/entries      PUT|DELETE /api/entries/:id
POST /api/users/:id/copy-meal {from_date, to_date, meal?}
POST /api/users/:id/exercises    DELETE /api/exercises/:id
GET  /api/users/:id/weights?from&to   POST /api/users/:id/weights   PUT|DELETE /api/weights/:id
GET  /api/users/:id/summary?from&to
GET  /api/users/:id/export.csv   GET /api/backup (SQLite snapshot)
GET|PUT /api/settings
POST /mcp
```

## MCP tools

`list_users`, `get_nutrient_catalog`, `search_foods`, `get_food`, `search_online_foods`, `lookup_barcode`,
`import_online_food`, `create_food`, `update_food`, `create_recipe`, `log_food`, `quick_add`, `get_diary`,
`update_entry`, `delete_entry`, `copy_meal`, `log_exercise`, `log_weight`, `get_weights`, `get_goals`,
`set_goals`, `get_summary`.

The server instructions tell the model to search the local library first, then online, and to create a
food only when neither has it. `create_food` accepts any subset of nutrients by key or common alias
("kcal", "carbohydrates"), per 100 g/ml or per serving, plus named servings and search aliases.
Unknown nutrient keys return an error listing the valid ones, so the model can correct itself. The user
comes from the `user` argument, then `?user=` on the MCP URL, then the only user if there is just one.

## Testing

- Rust unit tests: nutrient math, unit conversion, trend, TDEE, USDA/OFF parsing (saved real responses), MCP
  protocol.
- Rust integration tests: the router against a temp DB and a mock external API.
- Playwright: the real binary on a temp DB with a fixture server standing in for USDA/OFF. Covers onboarding,
  user switching and isolation, foods, servings, recipes shared between users, logging, quick add, copy
  meal, exercise, budget and protein numbers, weights and units, trends charts, goal calculator, online
  import ("costco milk"), barcode, MCP changes showing up in the UI, mobile layout, and no console errors.
- Bug bash: `docs/BUGBASH.md`, `scripts/bugbash.sh` and the `/bugbash` pi prompt.

## Out of scope (for now)

Camera barcode scanning (browsers block the camera on plain-HTTP LAN origins; barcodes can be typed), auth,
meal photos, importing from other apps, water tracking.
