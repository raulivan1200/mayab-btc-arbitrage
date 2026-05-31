APP=mayab-arbitrage

.PHONY: run test build docker

run:
	go run ./cmd/$(APP)

test:
	go test ./...

build:
	go build -trimpath -ldflags="-s -w" -o $(APP) ./cmd/$(APP)

docker:
	docker compose up --build
