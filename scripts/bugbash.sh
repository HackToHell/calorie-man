#!/usr/bin/env bash
# Bug bash: build, run every automated check, sweep the UI, fuzz the API/MCP, then start a seeded demo
# server for hands-on exploration. Writes bugbash/reports/<timestamp>/REPORT.md.
#
#   scripts/bugbash.sh             full run, leaves the demo server running on :8585
#   scripts/bugbash.sh --no-serve  stop when the report is written
#   scripts/bugbash.sh --live      also check the parsers against the real USDA / Open Food Facts APIs
#   scripts/bugbash.sh --quick     skip clippy, unit and e2e suites (sweep + fuzz only)
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

SERVE=1 LIVE=0 QUICK=0
for a in "$@"; do
  case "$a" in
    --no-serve) SERVE=0 ;;
    --live) LIVE=1 ;;
    --quick) QUICK=1 ;;
    *) echo "unknown option $a" >&2; exit 2 ;;
  esac
done

STAMP="$(date +%Y%m%d-%H%M%S)"
OUT="$ROOT/bugbash/reports/$STAMP"
mkdir -p "$OUT/logs"
STEPS="$OUT/steps.tsv"
: > "$STEPS"
PORT="${BUGBASH_PORT:-8585}"

step() { # step <name> <command...>
  local name="$1"; shift
  local log="$OUT/logs/$name.log" start end status
  printf '\n\033[1m▶ %s\033[0m\n' "$name"
  start=$(date +%s)
  ( "$@" ) > "$log" 2>&1
  status=$?
  end=$(date +%s)
  tail -n 15 "$log"
  printf '%s\t%s\t%s\n' "$name" "$status" "$((end - start))" >> "$STEPS"
  [ $status -eq 0 ] && printf '\033[32m✓ %s (%ss)\033[0m\n' "$name" "$((end - start))" || printf '\033[31m✗ %s (exit %s, see %s)\033[0m\n' "$name" "$status" "$log"
  return 0
}

step build-ui bash -c "cd web && npm run build"
step build-server cargo build
if [ $QUICK -eq 0 ]; then
  step fmt cargo fmt --check
  step clippy cargo clippy --all-targets -- -D warnings
  step svelte-check bash -c "cd web && npx svelte-check --tsconfig ./tsconfig.json --fail-on-warnings"
  step rust-tests cargo test
  step e2e bash -c "cd web && npx playwright test --reporter=line"
fi
step sweep-and-fuzz bash -c "cd web && BUGBASH_OUT='$OUT' npx playwright test --config playwright.bugbash.config.ts"

if [ $LIVE -eq 1 ]; then
  live() {
    set -e
    local db; db="$(mktemp -d)/live.db"
    ./target/debug/calorie-man serve --db "$db" --bind 127.0.0.1:18599 > "$OUT/logs/live-server.log" 2>&1 &
    local pid=$!
    trap 'kill $pid 2>/dev/null' RETURN
    sleep 1
    curl -sf -X POST localhost:18599/api/users -H 'content-type: application/json' -d '{"name":"Live"}' > /dev/null
    echo "USDA + OFF search for 'costco milk':"
    curl -sf "localhost:18599/api/external/search?q=costco%20milk&limit=5" | python3 -c '
import json,sys; d=json.load(sys.stdin)
for r in d["results"]: print(" ", r["source"], r["source_id"], r["name"], "|", r["brand"], "|", r["nutrients"].get("calories"), "kcal per", r["basis_amount"], r["basis_unit"])
print("  errors:", d["errors"]); assert d["results"], "no results"'
    echo "Barcode 3017624010701 (Nutella, OFF):"
    curl -sf "localhost:18599/api/external/barcode/3017624010701" | python3 -c '
import json,sys; d=json.load(sys.stdin); c=d["candidate"]; print(" ", c["name"], c["nutrients"]["calories"]); assert c["nutrients"]["calories"] > 400'
    echo "Import USDA 173944 (banana, full vitamins):"
    curl -sf -X POST localhost:18599/api/external/import -H 'content-type: application/json' -d '{"source":"usda","id":"173944"}' | python3 -c '
import json,sys; d=json.load(sys.stdin)["food"]; print(" ", d["name"], len(d["nutrients"]), "nutrients,", len(d["servings"]), "servings"); assert len(d["nutrients"]) > 20'
  }
  step live-apis live
fi

# Official MCP Inspector CLI against a seeded server: proves a real MCP client can talk to us.
mcp_inspector() {
  set -e
  local db; db="$(mktemp -d)/mcp.db"
  ./target/debug/calorie-man seed-demo --db "$db" --days 7 > /dev/null
  CALORIE_MAN_OFFLINE=1 ./target/debug/calorie-man serve --db "$db" --bind 127.0.0.1:18598 > "$OUT/logs/mcp-server.log" 2>&1 &
  local pid=$!
  trap 'kill $pid 2>/dev/null' RETURN
  sleep 1
  local url="http://127.0.0.1:18598/mcp?user=Alex"
  npx -y @modelcontextprotocol/inspector --cli "$url" --transport http --method tools/list | python3 -c 'import json,sys; t=json.load(sys.stdin)["tools"]; print(len(t), "tools"); assert len(t) > 15'
  npx -y @modelcontextprotocol/inspector --cli "$url" --transport http --method tools/call --tool-name get_diary | head -c 400; echo
  npx -y @modelcontextprotocol/inspector --cli "$url" --transport http --method tools/call --tool-name search_foods --tool-arg query=oats | head -c 400; echo
}
step mcp-inspector mcp_inspector

node web/scripts/bugbash-report.mjs "$OUT" > "$OUT/REPORT.md"
printf '\n\033[1mReport: %s\033[0m\n' "$OUT/REPORT.md"
sed -n '1,40p' "$OUT/REPORT.md"

if [ $SERVE -eq 1 ]; then
  DB="$OUT/demo.db"
  ./target/debug/calorie-man seed-demo --db "$DB" --days 90 > /dev/null
  printf '\n\033[1mDemo server for manual exploration\033[0m (Ctrl-C to stop)\n'
  printf '  Web UI: http://localhost:%s/   (phones on the LAN: http://%s:%s/)\n' "$PORT" "$(ipconfig getifaddr en0 2>/dev/null || hostname -I 2>/dev/null | awk '{print $1}')" "$PORT"
  printf '  MCP:    http://localhost:%s/mcp?user=Alex\n' "$PORT"
  printf '  DB:     %s (throwaway)\n' "$DB"
  printf '  Charters and the bug template are in docs/BUGBASH.md; log bugs in %s/BUGS.md\n\n' "$OUT"
  exec ./target/debug/calorie-man serve --db "$DB" --bind "0.0.0.0:$PORT"
fi
