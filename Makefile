# Eat: common tasks. Requires Rust (rustup) and Node 20+.
DB ?= data/calorie-man.db
BIND ?= 0.0.0.0:8484
ARGS ?=

.PHONY: all build release run dev seed test test-rust test-e2e check bugbash clean

all: build

web/node_modules: web/package.json
	cd web && npm ci && npx playwright install chromium
	touch web/node_modules

ui: web/node_modules
	cd web && npm run build

build: ui
	cargo build

# Single self-contained binary with the UI embedded: target/release/calorie-man
release: ui
	cargo build --release

run: build
	./target/debug/calorie-man serve --db $(DB) --bind $(BIND)

# Backend on :8484 plus the Vite dev server (hot reload) on :5173
dev: build
	(./target/debug/calorie-man serve --db $(DB) --bind 127.0.0.1:8484 &) ; cd web && npm run dev

seed: build
	./target/debug/calorie-man seed-demo --db data/demo.db --days 90 --force

check: web/node_modules
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cd web && npx svelte-check --tsconfig ./tsconfig.json --fail-on-warnings

test-rust:
	cargo test

test-e2e: build
	cd web && npx playwright test

test: check test-rust test-e2e

bugbash: web/node_modules
	scripts/bugbash.sh $(ARGS)

clean:
	cargo clean
	rm -rf web/dist web/test-results web/playwright-report
