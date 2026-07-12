#!/usr/bin/env sh
set -eu

BASE_URL="${BASE_URL:-http://localhost:8080}"
OUT_DIR="docs/evidencia"
mkdir -p "$OUT_DIR"

echo "Generando snapshot de evidencia sellada desde $BASE_URL..."

# 1. Paquete de evaluación (Scorecard, GA, estado actual)
curl -sS "$BASE_URL/api/paquete-evaluacion" -o "$OUT_DIR/paquete-evaluacion.json"
echo "✅ Paquete de evaluación guardado."

# 2. Export completo de auditoría (operaciones, eventos, rebalanceos, GA)
curl -sS "$BASE_URL/api/export/json" -o "$OUT_DIR/auditoria-completa.json"
echo "✅ Auditoría JSON guardada."

# 3. Export CSV
curl -sS "$BASE_URL/api/export/csv" -o "$OUT_DIR/auditoria-completa.csv"
echo "✅ Auditoría CSV guardada."

# 4. Benchmark de Latencias (Pipeline y Exchange)
curl -sS "$BASE_URL/api/latencias" -o "$OUT_DIR/benchmark-latencias.json"
echo "✅ Benchmark de latencias guardado."

# 5. Generar un Manifiesto Inmutable
cat <<EOF > "$OUT_DIR/manifest.json"
{
  "timestamp": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "commit": "$(git rev-parse HEAD 2>/dev/null || echo 'unknown')",
  "origen": "$BASE_URL"
}
EOF
echo "✅ Manifiesto generado."

echo ""
echo "=========================================================="
echo "🎯 Snapshot de evidencia generado en: $OUT_DIR/"
echo "Puedes empaquetar o hacer commit de este directorio para"
echo "probar que el motor funciona más allá del /tmp efímero."
echo "=========================================================="
