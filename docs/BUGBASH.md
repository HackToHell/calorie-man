# Bug bash

A bug bash is a time-boxed session (60-90 minutes) where everyone tries to break the app. It runs in three
parts: an automated sweep, hands-on exploration with charters, and a fix loop in which every bug gets a
regression test first.

## 1. Kick off

```sh
make bugbash            # = scripts/bugbash.sh
make bugbash ARGS=--live   # also hit the real USDA / Open Food Facts APIs
```

The script:

1. Builds the UI and the server.
2. Runs `cargo fmt --check`, `clippy -D warnings`, `svelte-check`, the Rust unit and API tests, and the
   Playwright E2E suite.
3. Runs the **sweep** (`web/tests/bugbash/`). Every screen, plus open dialogs, at desktop, tablet and
   phone sizes, in light and dark mode. For each one it records:
   - a full-page screenshot
   - console errors, page errors and 5xx responses
   - sideways scrolling and clipped text
   - `NaN`, `undefined` or `Infinity` in the page text
   - serious and critical WCAG 2 AA violations (axe)
4. Runs the **fuzzers**. Hostile values (huge numbers, negatives, `NaN`, 10k-character strings, SQL and
   HTML injection, emoji, impossible dates, wrong types) go to every REST endpoint and every MCP tool, plus
   malformed JSON-RPC and 120 concurrent writes. Pass criteria: no 5xx, JSON errors, the server stays
   healthy, totals stay correct.
5. Runs the official **MCP Inspector CLI** against a seeded server, which proves a third-party MCP client
   can list and call tools.
6. With `--live`, checks the parsers against today's real USDA and Open Food Facts responses (APIs change).
7. Writes `bugbash/reports/<timestamp>/REPORT.md` with step results, every finding, and links to the
   screenshots.
8. Starts a **seeded demo server** on `:8585` (two users, 90 days of history, a shared recipe) for the
   manual part. Its database is throwaway.

Triage the automated findings first. Then look at **every screenshot**: the checks do not catch ugly.

## 2. Explore (charters)

Pair up, take a charter, and spend 15-20 minutes on it. Test on a real phone on the LAN as well as on a
laptop. Each charter lists starting points, not a script; follow anything that looks off.

**A. Logging a day of food.** Log a realistic day for both people with every path: library search, recent
foods, online search, barcode, quick add, new food, a recipe. Change portions with every unit type (serving,
g, oz, ml, cup). Edit and delete entries, move them between meals and days, copy yesterday, copy from the
other person. *Do the totals match a calculator? Does anything need more taps than it should?*

**B. Foods and recipes (shared library).** Create foods per 100 g, per 100 ml and per serving. Add
servings, change the default, delete a serving that entries use. Build "Morning oats" with 4 ingredients,
log it for both people, then change an ingredient. *Do both diaries update? What happens to entries logged
with a deleted serving, an archived food, or a recipe whose ingredient was archived?*

**C. Online lookup.** Search "costco milk", "kirkland", "banana", "chicken breast", a brand name, a typo,
and something absurd. Import several. Look up barcodes from your pantry. Try it with the network off.
*Are the names and brands readable? Are the servings right? Do rate-limit and offline errors make sense?*

**D. Budgeting and goals.** Use the goal calculator. Change the goal mid-week and check that past days keep
their old budget. Turn "exercise adds to budget" on and off. Overeat on one day, undereat on another, and
read the weekly bank. *Is "banked" vs "over" obvious? Is the budget right on the Monday a week starts?*

**E. Protein.** Set a protein target from g/kg. Hit it exactly, miss it by 1 g, overshoot it. Check the
Today card, the Trends hit rate and the streak. *Does today (in progress) count against the streak?*

**F. Weight.** Log in kg, switch the profile to lb and back, log twice on one day, back-date a weigh-in, edit
one to a different date, delete one. Set a goal weight above and below the current weight. *Do the trend
line, rate and ETA stay sensible? Is the conversion exact both ways?*

**G. Trends.** Every range on the seeded data and on a brand-new user. Hover every chart. *Do averages
leave out unlogged days and today? Does the estimated maintenance agree with the profile estimate?*

**H. AI / MCP.** Connect a real client (Claude Desktop through `calorie-man mcp-stdio`, Claude Code, or any
Streamable HTTP client) to `http://<host>:8585/mcp?user=Alex` and have a conversation: "I had a Costco
rotisserie chicken thigh and a cup of Kirkland milk", "make my morning oats a recipe", "how is my protein
this week", "log 81.2 kg". *Does the AI find foods before creating duplicates? Are its nutrient numbers
sourced? Are tool errors clear enough for it to recover?*

**I. Two people, many devices.** Two browsers as two people at the same time. Switch users quickly. Edit
the same food from two places. *Does anyone ever see the other person's diary or goals?*

**J. Edges and resilience.** Midnight and time zones (set the profile time zone and ask the AI for
"today"), a daylight-saving week, emoji and very long names, 0.001 g and 99,999 kcal, the back button, a
server restart mid-session, the backup download and a restore.

## 3. Report

One row per bug in the report's table, with the details below it:

```markdown
### BUG-<n>: <one-line summary>
- Severity: S1 / S2 / S3 / S4
- Area: logging | foods | recipes | online | goals | protein | weight | trends | mcp | users | ui | data
- Where: <URL / tool / endpoint>, <viewport>, <light/dark>
- Steps:
  1. …
- Expected: …
- Actual: … (screenshot / log path)
- Regression test: <file::test name once written>
```

| Severity | Meaning | Example |
|---|---|---|
| S1 | Wrong or lost data, or a crash | Totals wrong; an entry saved for the wrong person; a 500 |
| S2 | A main flow is blocked or misleading | Cannot log on a phone; the budget shows the wrong goal |
| S3 | Works but confusing or ugly | Clipped labels; unclear error text; extra taps |
| S4 | Polish | Alignment, wording |

## 4. Fix loop

1. Fix S1 and S2 before the session ends. S3 and S4 go to the backlog unless they are trivial.
2. **Write the failing test first**: a Rust test (`tests/api.rs` or a unit test) for server logic, or a
   Playwright test (`web/tests/e2e/`) for UI behavior. Name it after the bug.
3. Fix the bug, then run `make test`, then rerun `make bugbash ARGS="--quick --no-serve"`.
4. Mark the bug fixed in the report with the test name.
5. Summarize the session in `bugbash/sessions/<date>.md`: bugs, how each was found, its regression test,
   what was checked and works, and what was deferred. Reports are gitignored; session logs are committed.
   Example: `bugbash/sessions/2026-11-14-session-1.md`.

## Exit criteria

- Every automated step passes and the sweep has no findings, or each finding is triaged as accepted.
- No open S1 or S2 bugs.
- Every screenshot was reviewed by a person.
- Every charter was explored by someone.
