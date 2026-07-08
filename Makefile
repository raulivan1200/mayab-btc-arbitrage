APP=mayab-arbitrage

.PHONY: run test check smoke build docker

run:
	cargo run

test:
	cargo test

check:
	cargo fmt -- --check
	cargo clippy -- -D warnings
	cargo test
	node --check internal/webui/web/app.js

smoke:
	./scripts/smoke-demo.sh

build:
	cargo build --release
	cp target/release/$(APP) ./$(APP)

docker:
	docker compose up --build
