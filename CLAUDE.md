# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Calorie Man: a household calorie/macro/micronutrient tracker for a LAN. One Rust binary (axum + rusqlite)
serves a Svelte 5 UI (embedded), a JSON REST API under `/api`, and an MCP endpoint at `/mcp`. Data is one
SQLite file. No auth by design. Design rationale, data model and full REST route list live in
`docs/PLAN.md`; MCP tool docs in `docs/MCP.md`; bug bash workflow in `docs/BUGBASH.md`.

## Commands

Requires Rust and Node 20+. The Makefile is the entry point.

```sh
make build          # npm ci + vite build (web/dist), then cargo build
make dev            # backend on 127.0.0.1:8484 + Vite hot reload on :5173 (proxies /api and /mcp)
make seed           # demo DB at data/demo.db (2 users, 90 days); serve it with --db data/demo.db
make check          # cargo fmt --check, clippy -D warnings, svelte-check --fail-on-warnings
make test-rust      # cargo test (unit tests + tests/api.rs integration)
make test-e2e       # Playwright; builds first
make test           # check + test-rust + test-e2e
make bugbash        # scripts/bugbash.sh: sweep, fuzz, MCP inspector, report (ARGS=--live|--quick|--no-serve)
```

Single tests:

```sh
cargo test --test api <name_substring>      # one integration test
cargo test --lib <module_or_test_name>      # unit tests inside src/
cd web && npx playwright test tests/e2e/logging.spec.ts -g "quick add"
cd web && npx playwright test --project=phone   # phone.spec only runs in the "phone" project
```

Playwright needs `target/debug/calorie-man` (or `CALORIE_MAN_BIN`) **and** `web/dist` to exist; global
setup fails otherwise. Each test spawns its own server on a temp DB and a free port, pointed at a mock
USDA/OFF server (`web/tests/mock-external.mjs`, port 18601) via `CALORIE_MAN_USDA_BASE`,
`CALORIE_MAN_OFF_BASE`, `CALORIE_MAN_OFF_SEARCH_BASE`.

`cargo build` also needs `web/dist` to exist (rust-embed). In debug builds the assets are read from disk,
so `npm run build` in `web/` is enough to see UI changes without recompiling Rust.

## Architecture

- **Nutrient catalog drives everything.** `src/nutrients.rs` lists every nutrient once (key, unit, DV,
  limit flag, USDA ids, OFF key/factor, aliases). DB columns (`n_<key>` on `foods` and `entries`), API
  validation, MCP schemas, external-API parsing and UI forms all derive from it. Adding a nutrient there
  auto-migrates the DB (`db::migrate` adds missing columns). NULL = unknown, not zero; coverage is reported.
- **Service layer is shared.** `src/service/*` holds all business logic. Both `src/api.rs` (REST) and
  `src/mcp.rs` (JSON-RPC 2.0 / MCP, stateless Streamable HTTP) call into it, so behaviour changes go in
  `service/` and should be tested through both interfaces where relevant.
- **SQLite access** goes through `AppState::db(|conn| ...)`, which runs on the blocking pool (r2d2, WAL,
  foreign keys on). Schema/migrations are in `src/db.rs`; non-catalog migrations are versioned by hand via
  `PRAGMA user_version` (currently 2).
- **External lookups** (`src/external.rs`): USDA FoodData Central and Open Food Facts, cached in the
  `external_cache` table for 7 days. "costco" is expanded to "kirkland". `CALORIE_MAN_OFFLINE=1`
  disables outbound requests. Parsers are tested against saved real responses in `tests/fixtures/`.
- **CLI** (`src/main.rs`): `serve` (default), `mcp-stdio` (stdio→HTTP bridge for stdio-only MCP clients),
  `seed-demo` (`src/seed.rs`).
- **Key data semantics** (see `docs/PLAN.md` for details):
  - Foods/recipes/servings are shared across users; diaries, goals, weights, exercise are per user.
  - Entries snapshot nutrients at log time; editing a food updates past entries by default. Deleting a food
    archives it.
  - Recipe nutrients = sum of ingredients; editing an ingredient recomputes dependent recipes; cycles rejected.
  - Goals are effective-dated (latest `effective_from <= day` applies). Weeks run Monday–Sunday.
  - Units: mass and volume never convert into each other (no density); named servings bridge them (`src/units.rs`).
  - Dates are local `YYYY-MM-DD` sent by the browser; MCP without a date uses the user's time zone.
  - MCP user resolution: `user` arg → `?user=` on the MCP URL → the only user if there is just one.
- **Web UI** (`web/`): Svelte 5 runes + TypeScript, hash routing, global state in `web/src/lib/state.svelte.ts`
  (current user and theme remembered in localStorage), API client in `web/src/lib/api.ts`. Charts are
  hand-written SVG (no chart library). Visual design follows the "Terrace" mockup in
  `docs/design/calorie-man-terrace-2a.html` (stepped `--terrace` clip-path corners, pastel macro colours:
  peach=calories, lilac=protein, butter=carbs, sea=fat, sky=fibre; Federo + Outfit fonts bundled in
  `web/public/fonts`; light/dark). Desktop has a sidebar, phones a bottom bar.

## Bug bash workflow

`.pi/prompts/bugbash.md` / `docs/BUGBASH.md`: run `scripts/bugbash.sh`, triage `bugbash/reports/<ts>/REPORT.md`
and its screenshots, and for every fix write a failing regression test first (`tests/api.rs` or
`web/tests/e2e/`), then fix, then `make test`. Session logs go in `bugbash/sessions/<date>-session-<n>.md`.
