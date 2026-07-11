#!/usr/bin/env sh
set -eu

SERVICE="${SERVICE:-mayab-btc-arbitrage}"
REGION="${REGION:-us-central1}"
PROJECT="${PROJECT:-$(gcloud config get-value project 2>/dev/null)}"
MIN_INSTANCES="${MIN_INSTANCES:-1}"
MAX_INSTANCES="${MAX_INSTANCES:-1}"
MEMORY="${MEMORY:-512Mi}"
CPU="${CPU:-1}"
CONCURRENCY="${CONCURRENCY:-20}"
TIMEOUT="${TIMEOUT:-3600}"

if [ -n "${IMAGE:-}" ]; then
  set -- --image "$IMAGE"
else
  set -- --source .
fi

gcloud run deploy "$SERVICE" \
  "$@" \
  --project "$PROJECT" \
  --region "$REGION" \
  --allow-unauthenticated \
  --memory "$MEMORY" \
  --cpu "$CPU" \
  --port 8080 \
  --concurrency "$CONCURRENCY" \
  --timeout "$TIMEOUT" \
  --min-instances "$MIN_INSTANCES" \
  --max-instances "$MAX_INSTANCES" \
  --execution-environment gen2 \
  --cpu-boost \
  --set-env-vars "RUST_LOG=error,DEMO_RENTABLE_INICIAL=false,FEE_BINANCE=0.0010,FEE_KRAKEN=0.0026,FEE_COINBASE=0.0060,FEE_OKX=0.0010,FEE_BYBIT=0.0010,RETIRO_BTC_BINANCE=0.00010,RETIRO_BTC_KRAKEN=0.00020,RETIRO_BTC_COINBASE=0.00012,RETIRO_BTC_OKX=0.00010,RETIRO_BTC_BYBIT=0.00010"

SERVICE_URL="$(gcloud run services describe "$SERVICE" \
  --project "$PROJECT" \
  --region "$REGION" \
  --format='value(status.url)')"

if [ -z "$SERVICE_URL" ]; then
  echo "No se pudo resolver la URL del servicio desplegado" >&2
  exit 1
fi

echo "Validando revision publica en ${SERVICE_URL}"
curl -fsS --retry 8 --retry-delay 2 --retry-all-errors "${SERVICE_URL}/api/healthz" >/dev/null
curl -fsS --retry 4 --retry-delay 2 --retry-all-errors "${SERVICE_URL}/api/preflight" >/dev/null

echo "Deploy validado: ${SERVICE_URL}"
