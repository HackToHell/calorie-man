# Session 2: "Terrace" redesign (docs/design/calorie-man-terrace-2a.html)

Adopted the design: Federo + Outfit (bundled), the plum, cream and pastel palette in light and dark,
stepped "terrace" corners, an octagon + button, diamond markers. Desktop has a sidebar and the Today
dashboard: calories and macros gauge with a fibre band, macro-split pyramid, micronutrient radar, this
week with stepped bar caps, a weight step chart, and a month calendar with a streak. Phones get the
light layout with the next-meal card and a bottom bar.
Final run: `bugbash/reports/20261114-184611`: all steps pass; axe now runs in both themes.

## Found and fixed during the redesign

| # | Sev | Summary | How found | Test |
|---|---|---|---|---|
| 1 | S2 | Phone + button was invisible: `.tabbar button` (more specific) overrode `.fab` | screenshot review | `e2e/phone.spec.ts` (uses the + button via `openAdd`) |
| 2 | S3 | Every stepped corner used 8px steps: `--terrace` defined on `:root` resolved `var(--t)` there, so 3/5px steps on buttons and inputs were ignored (small chips looked like crosses) | screenshot review | visual (sweep screenshots) |
| 3 | S3 | Gauge centre number overlapped the inner rings for 4+ digits ("1,288") | screenshot review | visual |
| 4 | S3 | Weight sparkline scaled with card width on tablets (huge axis labels); labels overlapped the line | screenshot review | visual |
| 5 | S3 | Ring-style focus/border on clipped inputs broke into fragments at the corner steps | screenshot review | visual |
| 6 | S3 | Next-meal suggestion used only the *last* meal a food was eaten at, and suggested side items (spinach) | E2E | `e2e/phone.spec.ts › next-meal card…`; server now returns `meal_uses` |
| 7 | S3 | After restoring an archived food the list stayed on "Archived", so the food seemed to vanish | E2E | `e2e/foods.spec.ts › deleting a logged food…` |
| 8 | S3 | No way to switch person on a phone without the sidebar | review | `bugbash/edge.spec.ts › rapid user switching` (phone uses the header avatar) |

## Behaviour changes

- Weeks briefly started on Sunday to match the design's SUN..SAT chart; switched back to Monday at the
  user's request in session 3.
- `diary.streak` (days in a row within budget, or logged when there is no goal) feeds the sidebar box
  and the calendar.
- Trend stats ignore future days.

## Design elements not built at first

- Meal timing "sun path": approved and built in session 3.
