APP=mayab-arbitrage

.PHONY: run test check build docker

run:
	cargo run

test:
	cargo test

check:
	cargo check --all-targets

build:
	cargo build --release
	cp target/release/$(APP) ./$(APP)

docker:
	docker compose up --build
