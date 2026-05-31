#!/usr/bin/env sh
set -eu

if command -v docker >/dev/null 2>&1; then
  if docker compose version >/dev/null 2>&1; then
    docker compose up --build
    exit 0
  fi
  if command -v docker-compose >/dev/null 2>&1; then
    docker-compose up --build
    exit 0
  fi
  docker build -t mayab-btc-arbitrage .
  docker run --rm -p 8080:8080 --env PORT=8080 mayab-btc-arbitrage
  exit 0
fi

if command -v go >/dev/null 2>&1; then
  PORT="${PORT:-8080}" go run ./cmd/mayab-arbitrage
  exit 0
fi

echo "Instala Docker o Go para ejecutar Mayab Arbitraje BTC." >&2
exit 1
