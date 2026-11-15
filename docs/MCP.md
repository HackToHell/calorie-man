# MCP guide

Eat exposes a [Model Context Protocol](https://modelcontextprotocol.io) server so an AI assistant can
manage the food library and diaries.

- **Transport:** Streamable HTTP at `POST /mcp`. Stateless (no session id); responses are
  `application/json`. `GET /mcp` returns 405, which the spec allows for servers without server-initiated
  messages. JSON-RPC batches are accepted.
- **Protocol versions:** 2025-06-18, 2025-03-26 and 2024-11-05 (the client's version is echoed if
  supported).
- **stdio:** `calorie-man mcp-stdio --url http://host:8484/mcp` bridges newline-delimited JSON-RPC on stdin and
  stdout to the HTTP endpoint.
- **Origin check:** browser requests must come from the server's own origin, localhost, or an entry in
  `CALORIE_MAN_ALLOWED_ORIGINS`. Requests without an `Origin` header (normal MCP clients) are allowed.
- **Default user:** `?user=<name or id>` on the URL. Tools that act on a diary also accept `user`. With
  one user in the database it is picked automatically.

The `initialize` result includes instructions for the model: search the library first, then online
databases, and create a food only when neither has it.

## Tools

| Tool | What it does |
|---|---|
| `list_users` | Users, units, today's date in their time zone, current goals |
| `get_nutrient_catalog` | Nutrient keys, units, daily values, aliases |
| `search_foods` | Search the shared library (foods and recipes) |
| `get_food` | Every nutrient, servings and ingredients of one food |
| `search_online_foods` | USDA FoodData Central + Open Food Facts candidates (`usda_kind`: generic/branded) |
| `lookup_barcode` | Library, then Open Food Facts, then USDA branded |
| `import_online_food` | Copy a candidate into the library (idempotent; add `aliases`) |
| `create_food` | New food from label values: any nutrient subset, per 100 g/ml or per serving, named servings |
| `update_food` | Partial update; nutrients merge key by key; recalculates past entries unless `update_past_entries: false` |
| `delete_food` | Delete, or archive if it was used |
| `create_recipe` | Recipe from library foods; `servings` yield and optional `cooked_weight_g` |
| `log_food` | Log a food or recipe with a quantity and unit (serving label, g/oz/lb, ml/cup/tbsp/fl oz, `serving`) and optionally the `time` it was eaten |
| `quick_add` | Log nutrients without a food (also takes `time`) |
| `get_diary` | A day's entries (with times), totals, budget, protein target, weekly bank, `meal_times` (sittings), `usual_meal_times` and `eating_window` (`include_micros` for vitamins) |
| `update_entry` / `delete_entry` | Change (including `time`, `null` clears it) or remove an entry |
| `copy_meal` | Copy a day or meal, optionally from another user (shared dinner) |
| `log_exercise` | Calories burned |
| `log_weight` / `get_weights` | Weigh-ins (kg or lb) with the smoothed trend |
| `get_goals` / `set_goals` | Effective-dated goals; unspecified fields carry over; includes TDEE estimates |
| `get_summary` | Averages, budget adherence, protein hit rate and streak, weight rate, estimated maintenance, micronutrient averages |

Dates accept `YYYY-MM-DD`, `today`, `yesterday` or an offset like `-2`. Meals: `breakfast`, `lunch`,
`dinner`, `snacks` (default from the time of day in the user's time zone).

Times (`time`) accept `07:40`, `19:30`, `7:40 pm`, `7pm` or `now`, and are stored as the user's local
`HH:MM`. Without `time`, today's entries get the current time when the meal fits the time of day
(breakfast 4-10, lunch 11-15, dinner 16-21, snacks otherwise). A back-filled meal (breakfast logged at 9 pm)
or any other day stays without a time. The model is told to pass the time whenever the user mentions it.

## Error handling

Problems with arguments come back as tool results with `isError: true` and a message the model can act on.
Examples:

- `unknown nutrient key(s): protien. Valid keys (with units): calories (kcal), protein (g), …`
- `unit 'cup' does not work for 'Protein bar'. Options: serving, 'bar' (serving_id 12, = 1 serving)`
- `barcode 0123… already belongs to food 7 'Protein bar'; use or update that food instead`
- `which user? Pass \`user\`: Alex, Sam`

Calling an unknown tool returns JSON-RPC error `-32602`.

## Example session

```text
user: I had a cup of Costco milk with breakfast
→ search_foods {query: "costco milk"}                      count 0
→ search_online_foods {query: "costco milk"}               usda 2460407 Kirkland Signature Organic Whole Milk, 62 kcal/100 ml, cup = 240 ml
→ import_online_food {source: "usda", id: "2460407", aliases: "costco milk"}
→ log_food {food_id: 31, quantity: 1, unit: "cup", meal: "breakfast"}
                                                           149 kcal, 8 g protein; 1,851 kcal and 142 g protein left
```
