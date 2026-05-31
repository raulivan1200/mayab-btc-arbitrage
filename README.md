# Mayab BTC Arbitrage

Mayab BTC Arbitrage es una web app en Go para monitorear order books de BTC en tiempo real, detectar arbitraje entre exchanges y simular ejecuciones con fees, slippage, costos de retiro amortizados, latencia y balances por wallet.

El nombre Mayab hace referencia al sureste mexicano y la interfaz toma una direccion visual editorial de alto contraste: reticula arquitectonica, tipografia grande y paneles densos de operacion. No reutiliza textos, imagenes ni assets de terceros.

## Ventaja principal

La ventaja fuerte es que todo corre como un solo binario Go: feeds WebSocket concurrentes, motor de decision, simulador de wallets, API y frontend embebido. Eso reduce latencia operativa, simplifica despliegue y evita depender de una cadena pesada de servicios para demostrar el sistema en vivo.

## Diferenciadores tecnicos

- Cinco exchanges conectados en paralelo: Binance, Kraken, Coinbase, OKX y Bybit.
- Evaluacion de rutas compra-venta en cada ciclo, no solo comparacion de dos mercados fijos.
- Simulacion realista con fees por exchange, slippage, retiro amortizado, riesgo de latencia, liquidez de top-of-book y balances por wallet.
- Ordenes parciales cuando la liquidez o el balance no cubren el tamano objetivo.
- Dashboard operativo en tiempo real con mapa de rutas, P&L, latencia, oportunidades y ejecuciones.
- Docker listo para correr sin instalar Go en la maquina evaluadora.

## Capturas

![Dashboard de Mayab BTC Arbitrage](screenshots/dashboard-desktop.jpg)

![Vista movil de Mayab BTC Arbitrage](screenshots/dashboard-mobile.jpg)

## Que hace

- Conecta feeds publicos WebSocket de Binance, Kraken, Coinbase, OKX y Bybit.
- Normaliza Bid, Ask, cantidad disponible, timestamp y latencia por exchange.
- Evalua todas las rutas compra-venta posibles entre exchanges.
- Calcula rentabilidad bruta y neta considerando trading fees, slippage, retiro amortizado y riesgo de latencia.
- Simula ejecuciones parciales cuando no hay liquidez o balance suficiente.
- Mantiene wallets por exchange y P&L acumulado.
- Expone dashboard web en tiempo real con mapa de rutas, tablas, balances y graficas.

## Tecnologias utilizadas

- Go 1.26
- Goroutines y canales para feeds concurrentes
- Gorilla WebSocket para conexiones de mercado y streaming al navegador
- HTML, CSS y JavaScript sin framework ni build step
- Canvas 2D para graficas y mapa de arbitraje
- Docker y Docker Compose para ejecucion reproducible

## Arquitectura

```text
cmd/mayab-arbitrage       entrada del binario
internal/mercado          conectores WebSocket por exchange
internal/motor            analizador, simulador, wallets y metricas
internal/http             API, WebSocket local y servidor estatico
internal/webui/web        frontend embebido en el binario Go
```

El servidor mantiene una goroutine por exchange y un loop de analisis. La UI recibe snapshots por `/tiempo-real` y puede consultar el estado completo en `/api/estado`.

## Ejecucion rapida con Docker

Solo necesitas Docker:

```bash
./scripts/run.sh
```

O directamente:

```bash
docker-compose up --build
```

Abre:

```text
http://localhost:8080
```

## Ejecucion local con Go

```bash
go mod download
go run ./cmd/mayab-arbitrage
```

Pruebas:

```bash
go test ./...
```

Build:

```bash
go build -trimpath -ldflags="-s -w" -o mayab-arbitrage ./cmd/mayab-arbitrage
```

## Configuracion

Puedes ajustar el perfil de costos con variables de entorno:

```bash
MAX_OPERACION_BTC=0.18 \
MIN_UTILIDAD_USD=1.25 \
MIN_SPREAD_NETO_BPS=0.65 \
SLIPPAGE_BPS=0.35 \
RETIRO_AMORTIZADO_BPS=0.12 \
PORT=8080 \
go run ./cmd/mayab-arbitrage
```

Fees por exchange:

```bash
FEE_BINANCE=0.001
FEE_KRAKEN=0.0026
FEE_COINBASE=0.006
FEE_OKX=0.001
FEE_BYBIT=0.001
```

## Despliegue gratis

Render:

1. Sube este repo a GitHub.
2. En Render crea un nuevo Web Service desde el repo.
3. Render detecta `render.yaml`.
4. Usa el plan Free y espera el deploy.

Fly.io:

```bash
fly launch --copy-config
fly deploy
```

Cloud Run:

```bash
gcloud run deploy mayab-btc-arbitrage \
  --source . \
  --region us-central1 \
  --allow-unauthenticated
```

## Endpoints

```text
GET /              dashboard
GET /healthz       health check
GET /api/estado    snapshot JSON completo
WS  /tiempo-real   streaming del estado en vivo
```

## Nota de seguridad

El sistema no opera dinero real ni usa API keys privadas. Todas las operaciones son simuladas sobre datos publicos de mercado.
