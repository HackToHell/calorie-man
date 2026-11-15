---
description: Run a bug bash on the Eat app (automated sweep + charters + fix loop)
argument-hint: "[charter letters, e.g. A C H] [--live]"
---
Run a bug bash session on this repository following docs/BUGBASH.md. Focus charters: ${1:-all}.

1. Run `scripts/bugbash.sh --no-serve ${@:2}` and wait for it. Read the generated REPORT.md in
   bugbash/reports/<latest>/. Every failing step and every sweep/fuzz finding is a candidate bug.
2. Look at the screenshots in the report's screens/ folders with the read tool (desktop, tablet and
   phone; light and dark). Note anything misaligned, clipped, unreadable, empty or wrong.
3. Start a demo server (`./target/debug/calorie-man seed-demo --db /tmp/bb.db --days 90 --force` then
   `./target/debug/calorie-man serve --db /tmp/bb.db --bind 127.0.0.1:8585` in the background) and explore
   the charters with throwaway Playwright scripts in web/scripts/ (headless Chromium, desktop and Pixel 7
   viewports). For charter H, call the MCP tools over HTTP like an AI client would.
4. Write every bug in the report with the BUG template and a severity.
5. Fix every S1 and S2 (plus cheap S3s). For each one, write a failing regression test first
   (tests/api.rs or web/tests/e2e/), then fix, then run `make test`.
6. Rerun `scripts/bugbash.sh --quick --no-serve` and confirm the findings are gone.
7. Write the session log to bugbash/sessions/<date>-session-<n>.md (format: see the existing sessions) and
   finish with a summary: bugs by severity, fixed, deferred (and why), and the tests that were added.
