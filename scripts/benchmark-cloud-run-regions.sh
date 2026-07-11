#!/usr/bin/env sh
set -eu

# Benchmark real de latencia exchange -> instancia Cloud Run. Despliega el
# mismo digest en servicios temporales, deja calentar los feeds, captura
# percentiles de /api/latencias y elimina las replicas al terminar.

SOURCE_SERVICE="${SOURCE_SERVICE:-mayab-btc-arbitrage}"
SOURCE_REGION="${SOURCE_REGION:-us-central1}"
REGIONS="${REGIONS:-us-central1 us-east4 us-west1}"
SERVICE_PREFIX="${SERVICE_PREFIX:-mayab-region-bench}"
WARMUP_SECONDS="${WARMUP_SECONDS:-45}"
SAMPLES="${SAMPLES:-3}"
SAMPLE_INTERVAL_SECONDS="${SAMPLE_INTERVAL_SECONDS:-5}"
CLEANUP="${CLEANUP:-1}"
PROJECT="${PROJECT:-$(gcloud config get-value project 2>/dev/null)}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-${TMPDIR:-/tmp}/mayab-region-benchmark-${RUN_ID}}"
RAW_FILE="${OUTPUT_DIR}/samples.ndjson"
JSON_FILE="${OUTPUT_DIR}/benchmark.json"
CSV_FILE="${OUTPUT_DIR}/benchmark.csv"
SERVICES_FILE="${OUTPUT_DIR}/services.tsv"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Falta comando requerido: $1" >&2
    exit 127
  fi
}

require_cmd gcloud
require_cmd curl
require_cmd python3
require_cmd awk

case "$WARMUP_SECONDS:$SAMPLES:$SAMPLE_INTERVAL_SECONDS" in
  *[!0-9:]* | *::* | :* | *:)
    echo "WARMUP_SECONDS, SAMPLES y SAMPLE_INTERVAL_SECONDS deben ser enteros" >&2
    exit 2
    ;;
esac

if [ "$SAMPLES" -lt 1 ]; then
  echo "SAMPLES debe ser mayor o igual a 1" >&2
  exit 2
fi

mkdir -p "$OUTPUT_DIR"
: >"$RAW_FILE"
: >"$SERVICES_FILE"

IMAGE="${IMAGE:-$(gcloud run services describe "$SOURCE_SERVICE" \
  --project "$PROJECT" \
  --region "$SOURCE_REGION" \
  --format='value(spec.template.spec.containers[0].image)')}"

if [ -z "$IMAGE" ]; then
  echo "No se pudo resolver la imagen de ${SOURCE_SERVICE} en ${SOURCE_REGION}" >&2
  exit 1
fi

cleanup() {
  if [ "$CLEANUP" != "1" ] || [ ! -s "$SERVICES_FILE" ]; then
    return
  fi
  echo "Eliminando replicas temporales..."
  while IFS="$(printf '\t')" read -r region service _url; do
    [ -n "$service" ] || continue
    gcloud run services delete "$service" \
      --project "$PROJECT" \
      --region "$region" \
      --quiet >/dev/null 2>&1 || true
  done <"$SERVICES_FILE"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

echo "Mayab Cloud Run multi-region benchmark"
echo "- project: $PROJECT"
echo "- image: $IMAGE"
echo "- regions: $REGIONS"
echo "- warmup: ${WARMUP_SECONDS}s | samples: $SAMPLES"

for region in $REGIONS; do
  suffix=$(printf '%s' "$region" | tr -cd 'a-z0-9-')
  service="${SERVICE_PREFIX}-${suffix}"
  echo "Desplegando $service en $region..."
  gcloud run deploy "$service" \
    --project "$PROJECT" \
    --image "$IMAGE" \
    --region "$region" \
    --allow-unauthenticated \
    --memory 512Mi \
    --cpu 1 \
    --port 8080 \
    --concurrency 20 \
    --timeout 3600 \
    --min-instances 1 \
    --max-instances 1 \
    --execution-environment gen2 \
    --cpu-boost \
    --set-env-vars "RUST_LOG=error,DEMO_RENTABLE_INICIAL=false" \
    --quiet >/dev/null
  url=$(gcloud run services describe "$service" \
    --project "$PROJECT" \
    --region "$region" \
    --format='value(status.url)')
  printf '%s\t%s\t%s\n' "$region" "$service" "$url" >>"$SERVICES_FILE"
  curl -fsS --max-time 20 "${url}/api/preflight" >/dev/null
done

echo "Calentando feeds durante ${WARMUP_SECONDS}s..."
remaining=$WARMUP_SECONDS
while [ "$remaining" -gt 0 ]; do
  step=5
  if [ "$remaining" -lt "$step" ]; then
    step=$remaining
  fi
  sleep "$step"
  remaining=$((remaining - step))
  echo "- faltan ${remaining}s"
done

sample=1
while [ "$sample" -le "$SAMPLES" ]; do
  while IFS="$(printf '\t')" read -r region service url; do
    body="${OUTPUT_DIR}/${region}-${sample}.json"
    metrics=$(curl -fsS --max-time 30 \
      -o "$body" \
      -w '%{time_total} %{http_code}' \
      "${url}/api/latencias")
    total_seconds=$(printf '%s' "$metrics" | awk '{print $1}')
    http_code=$(printf '%s' "$metrics" | awk '{print $2}')
    python3 - "$region" "$service" "$url" "$sample" "$total_seconds" "$http_code" "$body" >>"$RAW_FILE" <<'PY'
import json
import sys
from datetime import datetime, timezone

region, service, url, sample, seconds, status, path = sys.argv[1:]
with open(path, encoding="utf-8") as fh:
    payload = json.load(fh)
print(json.dumps({
    "region": region,
    "service": service,
    "url": url,
    "sample": int(sample),
    "capturedAt": datetime.now(timezone.utc).isoformat(),
    "clientRttMs": float(seconds) * 1000.0,
    "httpStatus": int(status),
    "payload": payload,
}, separators=(",", ":")))
PY
  done <"$SERVICES_FILE"
  if [ "$sample" -lt "$SAMPLES" ]; then
    sleep "$SAMPLE_INTERVAL_SECONDS"
  fi
  sample=$((sample + 1))
done

python3 - "$RAW_FILE" "$JSON_FILE" "$CSV_FILE" "$PROJECT" "$IMAGE" "$RUN_ID" <<'PY'
import csv
import json
import statistics
import sys
from collections import defaultdict
from datetime import datetime, timezone

raw_path, json_path, csv_path, project, image, run_id = sys.argv[1:]
samples = [json.loads(line) for line in open(raw_path, encoding="utf-8") if line.strip()]
by_region = defaultdict(list)
for sample in samples:
    by_region[sample["region"]].append(sample)

def median(values):
    clean = [float(v) for v in values if v is not None]
    return round(statistics.median(clean), 3) if clean else 0.0

ranking = []
csv_rows = []
for region, region_samples in by_region.items():
    by_exchange = defaultdict(list)
    for sample in region_samples:
        for item in sample.get("payload", {}).get("exchanges", []):
            by_exchange[item.get("exchange", "desconocido")].append(item)

    exchanges = []
    for exchange, rows in sorted(by_exchange.items()):
        result = {
            "exchange": exchange,
            "averageMs": median(row.get("promedioMs") for row in rows),
            "p50Ms": median(row.get("p50Ms") for row in rows),
            "p95Ms": median(row.get("p95Ms") for row in rows),
            "p99Ms": median(row.get("p99Ms") for row in rows),
            "events": max((int(row.get("eventos") or 0) for row in rows), default=0),
        }
        exchanges.append(result)
        csv_rows.append({"region": region, **result})

    ranking.append({
        "region": region,
        "scoreP95Ms": median(item["p95Ms"] for item in exchanges),
        "medianP50Ms": median(item["p50Ms"] for item in exchanges),
        "medianP99Ms": median(item["p99Ms"] for item in exchanges),
        "clientRttMs": median(sample["clientRttMs"] for sample in region_samples),
        "exchangesMeasured": len(exchanges),
        "samples": len(region_samples),
        "exchanges": exchanges,
    })

ranking.sort(key=lambda row: (row["scoreP95Ms"] or float("inf"), row["clientRttMs"]))
for index, row in enumerate(ranking, 1):
    row["rank"] = index

result = {
    "name": "Mayab Cloud Run Multi-Region Latency Benchmark",
    "runId": run_id,
    "generatedAt": datetime.now(timezone.utc).isoformat(),
    "project": project,
    "image": image,
    "methodology": {
        "primaryMetric": "median regional p95 of exchange event-to-ingest latency",
        "secondaryMetric": "HTTP RTT from the machine running this script",
        "warning": "Results describe this run and network conditions; they are not a universal exchange SLA."
    },
    "winner": ranking[0]["region"] if ranking else None,
    "ranking": ranking,
}
with open(json_path, "w", encoding="utf-8") as fh:
    json.dump(result, fh, ensure_ascii=False, indent=2)
with open(csv_path, "w", newline="", encoding="utf-8") as fh:
    writer = csv.DictWriter(fh, fieldnames=[
        "region", "exchange", "averageMs", "p50Ms", "p95Ms", "p99Ms", "events"
    ])
    writer.writeheader()
    writer.writerows(csv_rows)

print(json.dumps({
    "winner": result["winner"],
    "ranking": [
        {"rank": row["rank"], "region": row["region"], "scoreP95Ms": row["scoreP95Ms"]}
        for row in ranking
    ]
}, ensure_ascii=False, indent=2))
PY

echo "Resultados:"
echo "- JSON: $JSON_FILE"
echo "- CSV:  $CSV_FILE"
