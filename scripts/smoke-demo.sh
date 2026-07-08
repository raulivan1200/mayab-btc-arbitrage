#!/usr/bin/env sh
set -eu

BASE_URL="${BASE_URL:-http://127.0.0.1:8080}"
TMP_DIR="${TMPDIR:-/tmp}/mayab-smoke-demo.$$"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

mkdir -p "$TMP_DIR"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Falta comando requerido: $1" >&2
    exit 127
  fi
}

json_get() {
  path="$1"
  out="$2"
  curl -fsS "$BASE_URL$path" -o "$out"
}

json_post() {
  path="$1"
  payload="$2"
  out="$3"
  curl -fsS -X POST "$BASE_URL$path" \
    -H "Content-Type: application/json" \
    -d "$payload" \
    -o "$out"
}

require_cmd curl
require_cmd python3

echo "Smoke Mayab contra $BASE_URL"

json_get "/healthz" "$TMP_DIR/healthz.json"
json_get "/api/preflight" "$TMP_DIR/preflight-inicial.json"
json_post "/api/ga/evolucionar" '{"usarReplaySiVacio":true,"muestras":96}' "$TMP_DIR/ga.json"
json_post "/api/demo" '{"escenario":"mercado_rentable"}' "$TMP_DIR/demo-rentable.json"
json_post "/api/demo" '{"escenario":"rebalanceo"}' "$TMP_DIR/demo-rebalanceo.json"
json_get "/api/estado" "$TMP_DIR/estado.json"
json_get "/api/paquete-evaluacion" "$TMP_DIR/paquete.json"
json_get "/api/resumen-llm" "$TMP_DIR/resumen.json"

python3 - "$TMP_DIR" <<'PY'
import json
import pathlib
import sys

tmp = pathlib.Path(sys.argv[1])

def load(name):
    return json.loads((tmp / name).read_text())

healthz = load("healthz.json")
preflight = load("preflight-inicial.json")
ga = load("ga.json")
demo = load("demo-rentable.json")
rebalanceo = load("demo-rebalanceo.json")
estado = load("estado.json")
paquete = load("paquete.json")
resumen = load("resumen.json")

errors = []

if healthz.get("ok") is not True:
    errors.append("/healthz no devolvio ok=true")

readiness = preflight.get("judgeReadiness") or {}
rubrica_preflight = readiness.get("rubricaOficial") or []
if len(rubrica_preflight) != 5:
    errors.append("/api/preflight no expone los 5 criterios de rubrica oficial")

if ga.get("ok") is not True or ga.get("generacion", 0) < 1:
    errors.append("/api/ga/evolucionar no activo una generacion valida")

if demo.get("ok") is not True:
    errors.append("/api/demo mercado_rentable fallo")

if rebalanceo.get("ok") is not True:
    errors.append("/api/demo rebalanceo fallo")

metricas = estado.get("metricas") or {}
genetico = estado.get("genetico") or {}
eventos = estado.get("eventosEjecucion") or []
if metricas.get("operaciones", 0) <= 0:
    errors.append("estado no contiene operaciones despues de mercado_rentable")
if metricas.get("utilidadAcumuladaUsd", 0) <= 0:
    errors.append("PnL simulado no es positivo despues de mercado_rentable")
if not genetico.get("activo"):
    errors.append("GA no quedo activo despues del smoke")
if not any(str(e.get("tipo", "")).startswith("demo") for e in eventos):
    errors.append("no hay eventos demo visibles en estado")
if metricas.get("rebalanceosTotales", 0) <= 0:
    errors.append("no hay rebalanceos visibles despues de demo rebalanceo")

rubrica = paquete.get("rubricaOficialComite") or []
if len(rubrica) != 5:
    errors.append("/api/paquete-evaluacion no incluye 5 criterios oficiales")
if min((item.get("puntaje", 0) for item in rubrica), default=0) < 90:
    errors.append("algun criterio oficial quedo por debajo de 90 puntos")
if paquete.get("puntajeTotal", 0) < 90:
    errors.append("puntajeTotal del paquete quedo por debajo de 90")

recomendaciones = paquete.get("recomendacionesParaGanar") or []
if not recomendaciones or "Estado listo" not in recomendaciones[0]:
    errors.append("paquete no quedo en recomendacion final de estado listo")

if not resumen.get("persistencia", {}).get("activa"):
    errors.append("/api/resumen-llm no reporta persistencia activa")

if errors:
    print("Smoke fallido:")
    for error in errors:
        print(f"- {error}")
    sys.exit(1)

print("Smoke OK")
print(f"- readiness inicial: {readiness.get('status')} ({readiness.get('passed')}/{readiness.get('total')})")
print(f"- operaciones: {metricas.get('operaciones')} | PnL: {metricas.get('utilidadAcumuladaUsd'):.2f} USD")
print(f"- GA generacion: {genetico.get('generacion')} | activo: {genetico.get('activo')}")
print(f"- rebalanceos: {metricas.get('rebalanceosTotales')}")
print(f"- paquete: {paquete.get('puntajeTotal'):.2f} | huella: {paquete.get('huellaAuditoria')}")
PY
