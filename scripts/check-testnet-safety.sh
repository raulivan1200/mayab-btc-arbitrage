#!/usr/bin/env sh
set -eu

SOURCE="mayab-arbitrage/src/testnet.rs"
MANIFEST="mayab-cli/Cargo.toml"

test -f "$SOURCE"
grep -q 'required-features = \["testnet-execution"\]' "$MANIFEST"
grep -q 'api-public.sandbox.exchange.coinbase.com' "$SOURCE"

routes="$(awk '/pub const OUTBOUND_ROUTE_ALLOWLIST/{capture=1} capture{print} capture && /^];/{exit}' "$SOURCE")"
printf '%s\n' "$routes" | grep -Eq '\("GET", "/accounts"\)'
printf '%s\n' "$routes" | grep -Eq '\("POST", "/orders"\)'
if printf '%s\n' "$routes" | grep -Eqi 'deposit|withdraw|transfer|wallet|address'; then
  echo "La allowlist contiene una superficie prohibida" >&2
  exit 1
fi

if rg -n 'COINBASE_(API_KEY|API_SECRET|PASSPHRASE)=' . \
  -g '!target/**' -g '!.git/**' -g '!scripts/check-testnet-safety.sh'; then
  echo "Se encontró una credencial no separada de producción" >&2
  exit 1
fi

echo "Contrato testnet seguro: feature, host y rutas verificados"
