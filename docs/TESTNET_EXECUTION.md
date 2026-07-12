# Fase 7: ejecución segura en Coinbase Exchange Sandbox

Esta superficie no forma parte del servidor público. Se compila como el binario
`mayab-testnet-executor` únicamente con `testnet-execution`; no abre puertos ni
comparte el motor, rutas Axum o configuración tolerante de la demo.

## Controles que fallan cerrados

- El único host aceptado en código es `api-public.sandbox.exchange.coinbase.com`.
- El arranque exige `TESTNET_EXECUTION_CONFIRM=COINBASE_SANDBOX_ONLY` y
  `COINBASE_SANDBOX_KEY_PERMISSIONS=view,trade` exactamente.
- Las credenciales usan nombres `COINBASE_SANDBOX_*`, distintos de producción.
- `TradingTransport` sólo ofrece cuentas, preflight de permisos, órdenes,
  consulta/cancelación y fills. El motor no recibe un cliente HTTP.
- La allowlist admite seis contratos de método/ruta. No existe una operación de
  depósitos, retiros, direcciones, wallets o transferencias.
- El preflight consulta perfiles y bloquea si la respuesta anuncia capacidad de
  transferencia. La declaración local View/Trade es obligatoria porque Exchange
  Sandbox no garantiza una introspección uniforme de scopes para todas las keys.
- El cliente desactiva redirects, requiere HTTPS y no registra body, headers,
  firmas, passphrase ni secretos.
- Cada orden queda limitada a 0.001 BTC y USD 25 de nocional; exceder cualquiera
  de esos topes bloquea el arranque.

## Ciclo mínimo

El ejecutor consulta perfiles y cuentas, envía una limit post-only pequeña con
`client_oid` determinista, consulta estado hasta fill o timeout, cancela si
vence, consulta fills, reconcilia cuentas y exporta exposición final. Cada paso
se escribe en un JSONL con hash encadenado; al terminar, un lector independiente
reabre y verifica toda la cadena.

Compilación y validación sin credenciales:

```bash
cargo test --workspace --features testnet-execution
./scripts/check-testnet-safety.sh
cargo build --release --bin mayab-testnet-executor --features testnet-execution
```

Ejecución controlada (los tres secretos pueden ser archivos montados):

```bash
export TESTNET_EXECUTION_CONFIRM=COINBASE_SANDBOX_ONLY
export COINBASE_SANDBOX_HOST=api-public.sandbox.exchange.coinbase.com
export COINBASE_SANDBOX_KEY_PERMISSIONS=view,trade
export COINBASE_SANDBOX_API_KEY_FILE=/var/run/secrets/mayab/api-key
export COINBASE_SANDBOX_API_SECRET_FILE=/var/run/secrets/mayab/api-secret
export COINBASE_SANDBOX_PASSPHRASE_FILE=/var/run/secrets/mayab/passphrase
export TESTNET_PRODUCT_ID=BTC-USD TESTNET_ORDER_SIDE=buy
export TESTNET_RUN_ID=change-ticket-123
export TESTNET_LIMIT_PRICE=1000.00 TESTNET_ORDER_SIZE=0.0001
export TESTNET_TIMEOUT_MS=15000 TESTNET_POLL_MS=1000
export TESTNET_LEDGER_PATH=/ledger/run.jsonl
cargo run --release --bin mayab-testnet-executor --features testnet-execution
```

El precio debe elegirse conscientemente para la prueba. `post_only` reduce la
posibilidad de ejecución inmediata, pero el sandbox puede llenarla; use capital
ficticio mínimo y confirme la exposición final del ledger.

## Cloud Run privado, Secret Manager e IP fija

Despliegue este binario como **otro Cloud Run Job**, nunca como revisión del
servicio público. Use una service account dedicada que sólo tenga
`roles/secretmanager.secretAccessor` sobre las tres versiones de secreto. Monte
cada secreto como archivo; no lo pase por `--set-env-vars`, build args o imagen.

```bash
gcloud builds submit --tag "$REGION-docker.pkg.dev/$PROJECT/$REPO/mayab-testnet:$REV" -f Dockerfile.testnet
gcloud run jobs deploy mayab-testnet-executor \
  --image "$REGION-docker.pkg.dev/$PROJECT/$REPO/mayab-testnet:$REV" \
  --region "$REGION" --service-account mayab-testnet@$PROJECT.iam.gserviceaccount.com \
  --vpc-connector mayab-testnet-egress --vpc-egress all-traffic \
  --set-secrets '/var/run/secrets/mayab/api-key=coinbase-sandbox-api-key:latest,/var/run/secrets/mayab/api-secret=coinbase-sandbox-api-secret:latest,/var/run/secrets/mayab/passphrase=coinbase-sandbox-passphrase:latest' \
  --set-env-vars 'TESTNET_EXECUTION_CONFIRM=COINBASE_SANDBOX_ONLY,COINBASE_SANDBOX_HOST=api-public.sandbox.exchange.coinbase.com,COINBASE_SANDBOX_KEY_PERMISSIONS=view\,trade,COINBASE_SANDBOX_API_KEY_FILE=/var/run/secrets/mayab/api-key,COINBASE_SANDBOX_API_SECRET_FILE=/var/run/secrets/mayab/api-secret,COINBASE_SANDBOX_PASSPHRASE_FILE=/var/run/secrets/mayab/passphrase'
```

El connector debe salir por una subred con Cloud NAT y una IP reservada. Registre
esa IP en la allowlist de la key sandbox antes de ejecutar. Verifique que no haya
otra ruta de egress y que el Job no tenga invocación pública.

## Rotación y revocación

1. Cree otra key sandbox View/Trade, sin Transfer, con la misma IP permitida.
2. Agregue nuevas versiones en Secret Manager y ejecute preflight/ciclo mínimo.
3. Promueva versiones sólo después de auditar el ledger.
4. Revoque la key anterior en Coinbase y deshabilite sus versiones de secretos.
5. Ante sospecha, pause/elimine el Job, revoque primero la key, deshabilite los
   secretos y conserve ledger y audit logs. Nunca copie secretos a tickets/logs.

Referencias oficiales: [Sandbox](https://docs.cdp.coinbase.com/exchange/introduction/sandbox),
[autenticación y permisos](https://docs.cdp.coinbase.com/exchange/rest-api/authentication) y
[prácticas de seguridad](https://docs.cdp.coinbase.com/get-started/authentication/security-best-practices).
