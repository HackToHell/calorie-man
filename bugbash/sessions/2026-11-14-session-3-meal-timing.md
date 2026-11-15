# Session 3: meal timing and Monday weeks

## Built

- `entries.eaten_at` ("HH:MM", user's local time). The v2 migration back-fills it from `created_at` for
  entries logged on their own day.
- API and MCP: `time` on log, quick add, and edit (`null` clears it); copies keep their time.
  Without `time`, today's entries get "now" only when the meal fits the time of day.
- Diary `timing`: sittings (same meal within 60 min), usual time per meal (median over 28 days, 3+ days),
  first and last time, count of entries without a time.
- UI: a Time field when logging (now for the current meal, the usual time for other meals, editable) and
  in the entry editor; times in the meal lists.
- The sun-path chart on Today (design position) and in Log food. It shows the "now" octagon, the dashed
  next-meal diamond ("Dinner · usually 7:30") and the eating window.
- Weeks are back to Monday start: the weekly bank, the dashboard week chart (MON..SUN) and the calendar.

## Tests

- Rust: `util::tests::times` (parsing "7:40 pm", "now", …),
  `tests/api.rs::meal_times_occasions_and_usual_times` (sittings, usual time, edit/clear, copy, defaults,
  MCP).
- Playwright (`e2e/timing.spec.ts`, browser clock fixed at 4:20 PM): diamonds, eating window, usual
  dinner time, time defaults and editing, clearing a time, the Monday week and calendar.

## Found while building

- A test expected breakfast to be 338 kcal, but one oats serving is ½ cup (40 g), not 50 g; the code was
  right.
