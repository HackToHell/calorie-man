# Calorie Man: common tasks. Requires Rust (rustup) and Node 20+.
DB ?= data/calorie-man.db
BIND ?= 0.0.0.0:8484
ARGS ?=

.PHONY: all build release run dev seed test test-rust test-e2e check bugbash clean demo-wasm demo demo-serve demo-test demo-publish

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

# --- Browser demo (GitHub Pages) --------------------------------------------------------------------
# The real backend compiled to WebAssembly, on an in-memory SQLite. Built locally, not in CI. Needs:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version <the wasm-bindgen version in Cargo.lock>
#   a clang that targets wasm32 (Apple's can't: brew install llvm), optionally binaryen's wasm-opt
WASM_CC ?= $(firstword $(wildcard /opt/homebrew/opt/llvm/bin/clang /usr/local/opt/llvm/bin/clang) clang)
WASM_AR ?= $(firstword $(wildcard /opt/homebrew/opt/llvm/bin/llvm-ar /usr/local/opt/llvm/bin/llvm-ar) llvm-ar)
WASM_PKG = web/src/demo/pkg

demo-wasm:
	CC_wasm32_unknown_unknown=$(WASM_CC) AR_wasm32_unknown_unknown=$(WASM_AR) \
		cargo build -p calorie-man-demo --target wasm32-unknown-unknown --profile demo
	wasm-bindgen --target web --no-typescript --out-dir $(WASM_PKG) --out-name backend \
		target/wasm32-unknown-unknown/demo/calorie_man_demo.wasm
	if command -v wasm-opt > /dev/null; then \
		wasm-opt -Oz --strip-debug --strip-producers -o $(WASM_PKG)/backend_bg.wasm $(WASM_PKG)/backend_bg.wasm; fi

# web/dist-demo: a static site that works from any path
demo: web/node_modules demo-wasm
	cd web && npx vite build --mode demo

# http://127.0.0.1:8686/calorie-man/, the way GitHub Pages serves it
demo-serve: demo
	cd web && node scripts/serve-demo.mjs

demo-test: demo
	cd web && npx playwright test -c playwright.demo.config.ts

# Commit web/dist-demo to the gh-pages branch and push it
demo-publish: demo-test
	scripts/publish-demo.sh

clean:
	cargo clean
	rm -rf web/dist web/dist-demo web/src/demo/pkg web/test-results web/playwright-report
