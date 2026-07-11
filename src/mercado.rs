//! Adaptadores WebSocket para feeds públicos de exchanges.
//!
//! Cada adaptador normaliza libros de órdenes a `Cotizacion`. Los parsers son
//! tolerantes a mensajes no relevantes y devuelven `None` cuando el payload no
//! contiene un snapshot útil.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use reqwest::Client;
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{
    motor::Motor,
    types::{Cotizacion, NivelOrden},
};

#[derive(Clone)]
struct Adaptador {
    nombre: &'static str,
    par: String,
    url: String,
    suscripcion: Option<Value>,
    parser: fn(&[u8], &mut LibroEstado) -> Option<Cotizacion>,
    rest: Option<RestFallback>,
}

#[derive(Clone)]
struct RestFallback {
    url: String,
    parser: fn(&[u8], &str) -> Option<Cotizacion>,
}

#[derive(Default)]
struct LibroEstado {
    par: String,
    bids: BTreeMap<i64, f64>,
    asks: BTreeMap<i64, f64>,
}

impl LibroEstado {
    fn new(par: &str) -> Self {
        Self {
            par: normalizar_par(par),
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    fn reset(&mut self, par: &str) {
        self.par = normalizar_par(par);
        self.bids.clear();
        self.asks.clear();
    }

    fn actualizar_bids(&mut self, niveles: &[NivelOrden]) {
        actualizar_lado(&mut self.bids, niveles);
    }

    fn actualizar_asks(&mut self, niveles: &[NivelOrden]) {
        actualizar_lado(&mut self.asks, niveles);
    }

    fn snapshot_bids(&self, max: usize) -> Vec<NivelOrden> {
        self.bids
            .iter()
            .rev()
            .take(max)
            .map(|(p, c)| NivelOrden {
                precio: escala_precio(*p),
                cantidad: *c,
            })
            .collect()
    }

    fn snapshot_asks(&self, max: usize) -> Vec<NivelOrden> {
        self.asks
            .iter()
            .take(max)
            .map(|(p, c)| NivelOrden {
                precio: escala_precio(*p),
                cantidad: *c,
            })
            .collect()
    }

    fn cotizacion(&self, evento_unix_ms: i64) -> Option<Cotizacion> {
        cotizacion(
            &self.par,
            self.snapshot_bids(10),
            self.snapshot_asks(10),
            evento_unix_ms,
        )
    }
}

/// Lanza una tarea Tokio por cada feed público configurado.
pub async fn start_feeds(motor: Arc<Motor>, par_base: String) {
    let client = Client::builder()
        .timeout(Duration::from_secs(3))
        .user_agent("mayab-arbitrage/0.1 public-market-data")
        .build()
        .unwrap_or_else(|_| Client::new());
    for adaptador in adaptadores(&par_base) {
        if adaptador.rest.is_some() {
            let motor = motor.clone();
            let client = client.clone();
            let rest_adaptador = adaptador.clone();
            tokio::spawn(async move {
                run_rest_fallback(rest_adaptador, motor, client).await;
            });
        }
        let motor = motor.clone();
        tokio::spawn(async move {
            run_feed(adaptador, motor).await;
        });
    }
}

async fn run_rest_fallback(adaptador: Adaptador, motor: Arc<Motor>, client: Client) {
    let inicio_jitter = rand::thread_rng().gen_range(0..=2_000);
    tokio::time::sleep(Duration::from_millis(inicio_jitter)).await;
    let mut backoff = Duration::from_secs(5);
    loop {
        if !motor
            .feed_necesita_fallback(adaptador.nombre, &adaptador.par)
            .await
        {
            backoff = Duration::from_secs(5);
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        }
        match obtener_rest(&adaptador, &client).await {
            Ok(mut cotizacion) => {
                cotizacion.exchange = adaptador.nombre.to_string();
                cotizacion.recibida_en = Utc::now();
                cotizacion.conectado = false;
                cotizacion.ultimo_mensaje = "rest_fallback".to_string();
                motor.recibir_cotizacion(cotizacion).await;
                backoff = Duration::from_secs(5);
                tracing::info!(
                    exchange = adaptador.nombre,
                    "snapshot REST usado como fallback"
                );
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Err(err) => {
                tracing::warn!(exchange = adaptador.nombre, error = %err, "fallback REST fallo");
                let jitter =
                    rand::thread_rng().gen_range(0..=backoff.as_millis().max(1) as u64 / 2);
                tokio::time::sleep(backoff + Duration::from_millis(jitter)).await;
                backoff = (backoff * 2).min(Duration::from_secs(60));
            }
        }
    }
}

async fn obtener_rest(adaptador: &Adaptador, client: &Client) -> anyhow::Result<Cotizacion> {
    let Some(rest) = &adaptador.rest else {
        anyhow::bail!("adaptador sin REST fallback");
    };
    let bytes = client
        .get(&rest.url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    (rest.parser)(&bytes, &adaptador.par)
        .ok_or_else(|| anyhow::anyhow!("payload REST sin libro util"))
}

async fn run_feed(adaptador: Adaptador, motor: Arc<Motor>) {
    let mut backoff = Duration::from_millis(650);
    loop {
        match conectar(&adaptador, motor.clone()).await {
            Ok(_) => backoff = Duration::from_millis(650),
            Err(err) => {
                tracing::warn!(exchange = adaptador.nombre, error = %err, "feed desconectado")
            }
        }
        let jitter = rand::thread_rng().gen_range(0..=backoff.as_millis().max(1) as u64 / 2);
        tokio::time::sleep(backoff + Duration::from_millis(jitter)).await;
        backoff = (backoff * 2).min(Duration::from_secs(8));
    }
}

async fn conectar(adaptador: &Adaptador, motor: Arc<Motor>) -> anyhow::Result<()> {
    let (mut ws, _) = connect_async(&adaptador.url).await?;
    let mut libro = LibroEstado::new(&adaptador.par);
    tracing::info!(exchange = adaptador.nombre, "feed conectado");
    if let Some(payload) = &adaptador.suscripcion {
        ws.send(Message::Text(payload.to_string())).await?;
    }
    let mut ping = tokio::time::interval(Duration::from_secs(20));
    loop {
        tokio::select! {
            _ = ping.tick() => {
                ws.send(Message::Ping(Vec::new())).await?;
            }
            msg = ws.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => recibir(adaptador, text.as_bytes(), &motor, &mut libro).await,
                    Some(Ok(Message::Binary(bytes))) => recibir(adaptador, &bytes, &motor, &mut libro).await,
                    Some(Ok(Message::Ping(payload))) => ws.send(Message::Pong(payload)).await?,
                    Some(Ok(Message::Close(_))) | None => anyhow::bail!("conexion cerrada"),
                    Some(Err(err)) => return Err(err.into()),
                    _ => {}
                }
            }
        }
    }
}

async fn recibir(adaptador: &Adaptador, bytes: &[u8], motor: &Motor, libro: &mut LibroEstado) {
    if let Some(mut cotizacion) = (adaptador.parser)(bytes, libro) {
        cotizacion.exchange = adaptador.nombre.to_string();
        cotizacion.recibida_en = Utc::now();
        motor.recibir_cotizacion(cotizacion).await;
    }
}

fn adaptadores(par: &str) -> Vec<Adaptador> {
    let base = activo_base(par);
    let usdt = format!("{base}USDT");
    let usd_dash = format!("{base}-USD");
    let usdt_dash = format!("{base}-USDT");
    let usd_slash = format!("{base}/USD");
    vec![
        Adaptador {
            nombre: "Binance",
            par: normalizar_par(&usdt),
            url: format!(
                "wss://data-stream.binance.vision/ws/{}@depth10@100ms",
                usdt.to_lowercase()
            ),
            suscripcion: None,
            parser: parsear_binance,
            rest: Some(RestFallback {
                url: format!("https://api.binance.com/api/v3/depth?symbol={usdt}&limit=10"),
                parser: parsear_rest_binance,
            }),
        },
        Adaptador {
            nombre: "Kraken",
            par: normalizar_par(&usd_slash),
            url: "wss://ws.kraken.com/v2".to_string(),
            suscripcion: Some(
                serde_json::json!({"method":"subscribe","params":{"channel":"book","symbol":[usd_slash],"depth":10,"snapshot":true}}),
            ),
            parser: parsear_kraken,
            rest: Some(RestFallback {
                url: format!("https://api.kraken.com/0/public/Depth?pair={base}USD&count=10"),
                parser: parsear_rest_kraken,
            }),
        },
        Adaptador {
            nombre: "Coinbase",
            par: normalizar_par(&usd_dash),
            url: "wss://advanced-trade-ws.coinbase.com".to_string(),
            suscripcion: Some(
                serde_json::json!({"type":"subscribe","product_ids":[usd_dash],"channel":"level2"}),
            ),
            parser: parsear_coinbase,
            rest: Some(RestFallback {
                url: format!("https://api.exchange.coinbase.com/products/{usd_dash}/book?level=2"),
                parser: parsear_rest_coinbase,
            }),
        },
        Adaptador {
            nombre: "OKX",
            par: normalizar_par(&usdt_dash),
            url: "wss://ws.okx.com:8443/ws/v5/public".to_string(),
            suscripcion: Some(
                serde_json::json!({"op":"subscribe","args":[{"channel":"books5","instId":usdt_dash}]}),
            ),
            parser: parsear_okx,
            rest: Some(RestFallback {
                url: format!("https://www.okx.com/api/v5/market/books?instId={usdt_dash}&sz=10"),
                parser: parsear_rest_okx,
            }),
        },
        Adaptador {
            nombre: "Bybit",
            par: normalizar_par(&usdt),
            url: "wss://stream.bybit.com/v5/public/spot".to_string(),
            suscripcion: Some(
                serde_json::json!({"op":"subscribe","args":[format!("orderbook.1.{usdt}")]}),
            ),
            parser: parsear_bybit,
            rest: Some(RestFallback {
                url: format!(
                    "https://api.bybit.com/v5/market/orderbook?category=spot&symbol={usdt}&limit=10"
                ),
                parser: parsear_rest_bybit,
            }),
        },
    ]
}

fn parsear_binance(bytes: &[u8], libro: &mut LibroEstado) -> Option<Cotizacion> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    let bids = niveles_strings(v.get("b").or_else(|| v.get("bids"))?.as_array()?, 10);
    let asks = niveles_strings(v.get("a").or_else(|| v.get("asks"))?.as_array()?, 10);
    if v.get("lastUpdateId").is_some() {
        let par_actual = libro.par.clone();
        libro.reset(&par_actual);
    }
    libro.actualizar_bids(&bids);
    libro.actualizar_asks(&asks);
    libro.cotizacion(v.get("E").and_then(Value::as_i64).unwrap_or_default())
}

fn parsear_okx(bytes: &[u8], libro: &mut LibroEstado) -> Option<Cotizacion> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    if !v
        .pointer("/arg/channel")
        .and_then(Value::as_str)?
        .starts_with("books")
    {
        return None;
    }
    let item = v.get("data")?.as_array()?.first()?;
    let par = v
        .pointer("/arg/instId")
        .and_then(Value::as_str)
        .unwrap_or("BTC-USDT");
    if v.get("action").and_then(Value::as_str) == Some("snapshot")
        || v.pointer("/arg/channel").and_then(Value::as_str) == Some("books5")
    {
        libro.reset(par);
    } else {
        libro.par = normalizar_par(par);
    }
    // Los deltas pueden modificar un solo lado del libro. Una clave ausente no
    // invalida el mensaje: equivale a "sin cambios" para ese lado.
    let bids = item
        .get("bids")
        .and_then(Value::as_array)
        .map(|niveles| niveles_strings(niveles, 10))
        .unwrap_or_default();
    let asks = item
        .get("asks")
        .and_then(Value::as_array)
        .map(|niveles| niveles_strings(niveles, 10))
        .unwrap_or_default();
    let ts = item
        .get("ts")
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or_default();
    libro.actualizar_bids(&bids);
    libro.actualizar_asks(&asks);
    libro.cotizacion(ts)
}

fn parsear_bybit(bytes: &[u8], libro: &mut LibroEstado) -> Option<Cotizacion> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    let topic = v.get("topic").and_then(Value::as_str)?;
    if !topic.starts_with("orderbook.") {
        return None;
    }
    let par = topic.rsplit('.').next().unwrap_or("BTCUSDT");
    if v.get("type").and_then(Value::as_str) == Some("snapshot") {
        libro.reset(par);
    } else {
        libro.par = normalizar_par(par);
    }
    let bids = v
        .pointer("/data/b")
        .and_then(Value::as_array)
        .map(|niveles| niveles_strings(niveles, 10))
        .unwrap_or_default();
    let asks = v
        .pointer("/data/a")
        .and_then(Value::as_array)
        .map(|niveles| niveles_strings(niveles, 10))
        .unwrap_or_default();
    libro.actualizar_bids(&bids);
    libro.actualizar_asks(&asks);
    libro.cotizacion(v.get("ts").and_then(Value::as_i64).unwrap_or_default())
}

fn parsear_kraken(bytes: &[u8], libro: &mut LibroEstado) -> Option<Cotizacion> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    if v.get("channel").and_then(Value::as_str)? != "book" {
        return None;
    }
    let item = v.get("data")?.as_array()?.first()?;
    let par = item
        .get("symbol")
        .and_then(Value::as_str)
        .unwrap_or("BTC/USD");
    if item.get("type").and_then(Value::as_str) == Some("snapshot")
        || v.get("type").and_then(Value::as_str) == Some("snapshot")
    {
        libro.reset(par);
    } else {
        libro.par = normalizar_par(par);
    }
    let bids = item
        .get("bids")
        .and_then(Value::as_array)
        .map(|niveles| niveles_mixtos(niveles, 10))
        .unwrap_or_default();
    let asks = item
        .get("asks")
        .and_then(Value::as_array)
        .map(|niveles| niveles_mixtos(niveles, 10))
        .unwrap_or_default();
    let ts = item
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(rfc3339_ms)
        .unwrap_or_default();
    libro.actualizar_bids(&bids);
    libro.actualizar_asks(&asks);
    libro.cotizacion(ts)
}

fn parsear_coinbase(bytes: &[u8], libro: &mut LibroEstado) -> Option<Cotizacion> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    if !matches!(
        v.get("channel").and_then(Value::as_str)?,
        "level2" | "l2_data"
    ) {
        return None;
    }
    let ts = v
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(rfc3339_ms)
        .unwrap_or_default();
    for event in v.get("events")?.as_array()? {
        if let Some(product_id) = event.get("product_id").and_then(Value::as_str) {
            if event.get("type").and_then(Value::as_str) == Some("snapshot") {
                libro.reset(product_id);
            } else {
                libro.par = normalizar_par(product_id);
            }
        }
        for update in event.get("updates")?.as_array()? {
            let product_id = update
                .get("product_id")
                .or_else(|| event.get("product_id"))
                .and_then(Value::as_str)
                .unwrap_or("BTC-USD");
            libro.par = normalizar_par(product_id);
            let precio = update
                .get("price_level")
                .or_else(|| update.get("price"))
                .and_then(parse_num)?;
            let cantidad = update
                .get("new_quantity")
                .or_else(|| update.get("quantity"))
                .and_then(parse_num)?;
            let nivel = NivelOrden { precio, cantidad };
            match update.get("side").and_then(Value::as_str) {
                Some("bid") | Some("BUY") | Some("buy") => libro.actualizar_bids(&[nivel]),
                Some("offer") | Some("ask") | Some("SELL") | Some("sell") => {
                    libro.actualizar_asks(&[nivel])
                }
                _ => {}
            }
        }
    }
    libro.cotizacion(ts)
}

fn parsear_rest_binance(bytes: &[u8], par: &str) -> Option<Cotizacion> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    let bids = niveles_strings(v.get("bids")?.as_array()?, 10);
    let asks = niveles_strings(v.get("asks")?.as_array()?, 10);
    cotizacion(par, bids, asks, 0)
}

fn parsear_rest_kraken(bytes: &[u8], par: &str) -> Option<Cotizacion> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    if !v.get("error")?.as_array()?.is_empty() {
        return None;
    }
    let book = v.get("result")?.as_object()?.values().next()?;
    let bids = niveles_strings(book.get("bids")?.as_array()?, 10);
    let asks = niveles_strings(book.get("asks")?.as_array()?, 10);
    cotizacion(par, bids, asks, 0)
}

fn parsear_rest_coinbase(bytes: &[u8], par: &str) -> Option<Cotizacion> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    let bids = niveles_strings(v.get("bids")?.as_array()?, 10);
    let asks = niveles_strings(v.get("asks")?.as_array()?, 10);
    let ts = v
        .get("time")
        .and_then(Value::as_str)
        .and_then(rfc3339_ms)
        .unwrap_or_default();
    cotizacion(par, bids, asks, ts)
}

fn parsear_rest_okx(bytes: &[u8], par: &str) -> Option<Cotizacion> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    if v.get("code").and_then(Value::as_str) != Some("0") {
        return None;
    }
    let book = v.get("data")?.as_array()?.first()?;
    let bids = niveles_strings(book.get("bids")?.as_array()?, 10);
    let asks = niveles_strings(book.get("asks")?.as_array()?, 10);
    let ts = book
        .get("ts")
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or_else(|| Utc::now().timestamp_millis());
    cotizacion(par, bids, asks, ts)
}

fn parsear_rest_bybit(bytes: &[u8], par: &str) -> Option<Cotizacion> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    if v.get("retCode").and_then(Value::as_i64) != Some(0) {
        return None;
    }
    let result = v.get("result")?;
    let bids = niveles_strings(result.get("b")?.as_array()?, 10);
    let asks = niveles_strings(result.get("a")?.as_array()?, 10);
    let ts = result
        .get("ts")
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
        })
        .or_else(|| v.get("time").and_then(Value::as_i64))
        .unwrap_or_else(|| Utc::now().timestamp_millis());
    cotizacion(par, bids, asks, ts)
}

fn niveles_strings(items: &[Value], max: usize) -> Vec<NivelOrden> {
    items
        .iter()
        .take(max)
        .filter_map(|item| {
            let arr = item.as_array()?;
            let precio = parse_num(arr.first()?)?;
            let cantidad = parse_num(arr.get(1)?)?;
            (precio > 0.0).then_some(NivelOrden { precio, cantidad })
        })
        .collect()
}

fn niveles_mixtos(items: &[Value], max: usize) -> Vec<NivelOrden> {
    items
        .iter()
        .take(max)
        .filter_map(|item| {
            let (precio, cantidad) = if let Some(arr) = item.as_array() {
                (parse_num(arr.first()?)?, parse_num(arr.get(1)?)?)
            } else {
                (
                    item.get("price").and_then(parse_num)?,
                    item.get("qty")
                        .or_else(|| item.get("quantity"))
                        .and_then(parse_num)?,
                )
            };
            (precio > 0.0).then_some(NivelOrden { precio, cantidad })
        })
        .collect()
}

fn parse_num(v: &Value) -> Option<f64> {
    match v {
        Value::String(s) => s.parse::<f64>().ok(),
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
    .filter(|n| n.is_finite())
}

fn actualizar_lado(lado: &mut BTreeMap<i64, f64>, niveles: &[NivelOrden]) {
    for nivel in niveles {
        let precio = llave_precio(nivel.precio);
        if precio <= 0 {
            continue;
        }
        if nivel.cantidad <= 0.0 || !nivel.cantidad.is_finite() {
            lado.remove(&precio);
        } else {
            lado.insert(precio, nivel.cantidad);
        }
    }
}

fn llave_precio(precio: f64) -> i64 {
    (precio * 100_000_000.0).round() as i64
}

fn escala_precio(precio: i64) -> f64 {
    precio as f64 / 100_000_000.0
}

fn cotizacion(
    par: &str,
    bids: Vec<NivelOrden>,
    asks: Vec<NivelOrden>,
    evento_unix_ms: i64,
) -> Option<Cotizacion> {
    let bid = bids.first()?;
    let ask = asks.first()?;
    Some(Cotizacion {
        exchange: String::new(),
        par: normalizar_par(par),
        bid: bid.precio,
        bid_cantidad: bid.cantidad,
        ask: ask.precio,
        ask_cantidad: ask.cantidad,
        bids,
        asks,
        evento_unix_ms,
        recibida_en: Utc::now(),
        latencia_ms: 0,
        secuencia: 0,
        conectado: true,
        ultimo_mensaje: String::new(),
    })
}

fn normalizar_par(par: &str) -> String {
    let compact = par.trim().to_ascii_uppercase().replace(['/', '-'], "");
    if let Some(base) = compact
        .strip_suffix("USDT")
        .or_else(|| compact.strip_suffix("USD"))
    {
        format!("{base}/USD")
    } else {
        par.to_ascii_uppercase()
    }
}

fn activo_base(par: &str) -> String {
    let compact = par.trim().to_ascii_uppercase().replace(['/', '-'], "");
    compact
        .strip_suffix("USDT")
        .or_else(|| compact.strip_suffix("USD"))
        .unwrap_or("BTC")
        .to_string()
}

fn rfc3339_ms(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|t| t.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_binance_depth() {
        let msg = br#"{"E":1710000000000,"b":[["100.0","2.0"]],"a":[["101.0","1.5"]]}"#;
        let mut libro = LibroEstado::new("BTC/USD");
        let c = parsear_binance(msg, &mut libro).unwrap();
        assert_eq!(c.par, "BTC/USD");
        assert_eq!(c.bid, 100.0);
        assert_eq!(c.ask, 101.0);
    }

    #[test]
    fn parsea_coinbase_l2_data() {
        let msg = br#"{"channel":"l2_data","timestamp":"2024-03-09T00:00:00Z","events":[{"type":"snapshot","product_id":"BTC-USD","updates":[{"side":"bid","price_level":"100.0","new_quantity":"2.0"},{"side":"offer","price_level":"101.0","new_quantity":"1.5"}]}]}"#;
        let mut libro = LibroEstado::new("BTC/USD");
        let c = parsear_coinbase(msg, &mut libro).unwrap();
        assert_eq!(c.par, "BTC/USD");
        assert_eq!(c.bid, 100.0);
        assert_eq!(c.ask, 101.0);
    }

    #[test]
    fn parsea_bybit_orderbook_con_profundidad_en_topic() {
        let msg = br#"{"topic":"orderbook.1.BTCUSDT","type":"snapshot","ts":1710000000000,"data":{"b":[["100.0","2.0"]],"a":[["101.0","1.5"]]}}"#;
        let mut libro = LibroEstado::new("BTC/USD");
        let c = parsear_bybit(msg, &mut libro).unwrap();
        assert_eq!(c.par, "BTC/USD");
        assert_eq!(c.bid, 100.0);
        assert_eq!(c.ask, 101.0);
    }

    #[test]
    fn bybit_conserva_ask_en_delta_solo_bid() {
        let snapshot = br#"{"topic":"orderbook.50.BTCUSDT","type":"snapshot","ts":1710000000000,"data":{"b":[["100.0","2.0"]],"a":[["101.0","1.5"]]}}"#;
        let delta = br#"{"topic":"orderbook.50.BTCUSDT","type":"delta","ts":1710000000001,"data":{"b":[["100.5","3.0"]]}}"#;
        let mut libro = LibroEstado::new("BTC/USD");
        parsear_bybit(snapshot, &mut libro).unwrap();

        let c = parsear_bybit(delta, &mut libro).unwrap();

        assert_eq!(c.bid, 100.5);
        assert_eq!(c.ask, 101.0);
    }

    #[test]
    fn kraken_conserva_bid_en_delta_solo_ask() {
        let snapshot = br#"{"channel":"book","type":"snapshot","data":[{"symbol":"BTC/USD","bids":[{"price":100.0,"qty":2.0}],"asks":[{"price":101.0,"qty":1.5}],"timestamp":"2024-03-09T00:00:00Z"}]}"#;
        let delta = br#"{"channel":"book","type":"update","data":[{"symbol":"BTC/USD","asks":[{"price":100.8,"qty":1.0}],"timestamp":"2024-03-09T00:00:00.001Z"}]}"#;
        let mut libro = LibroEstado::new("BTC/USD");
        parsear_kraken(snapshot, &mut libro).unwrap();

        let c = parsear_kraken(delta, &mut libro).unwrap();

        assert_eq!(c.bid, 100.0);
        assert_eq!(c.ask, 100.8);
    }

    #[test]
    fn okx_conserva_ask_en_delta_solo_bid() {
        let snapshot = br#"{"arg":{"channel":"books","instId":"BTC-USDT"},"action":"snapshot","data":[{"bids":[["100.0","2.0"]],"asks":[["101.0","1.5"]],"ts":"1710000000000"}]}"#;
        let delta = br#"{"arg":{"channel":"books","instId":"BTC-USDT"},"action":"update","data":[{"bids":[["100.5","3.0"]],"ts":"1710000000001"}]}"#;
        let mut libro = LibroEstado::new("BTC/USD");
        parsear_okx(snapshot, &mut libro).unwrap();

        let c = parsear_okx(delta, &mut libro).unwrap();

        assert_eq!(c.bid, 100.5);
        assert_eq!(c.ask, 101.0);
    }

    #[test]
    fn parsea_rest_depth_binance() {
        let msg = br#"{"lastUpdateId":1,"bids":[["100.0","2.0"]],"asks":[["101.0","1.5"]]}"#;
        let c = parsear_rest_binance(msg, "BTC/USD").unwrap();
        assert_eq!(c.bid_cantidad, 2.0);
        assert_eq!(c.ask_cantidad, 1.5);
    }

    #[test]
    fn parsea_rest_depth_kraken() {
        let msg = br#"{"error":[],"result":{"XXBTZUSD":{"bids":[["100.0","2.0","1"]],"asks":[["101.0","1.5","1"]]}}}"#;
        let c = parsear_rest_kraken(msg, "BTC/USD").unwrap();
        assert_eq!(c.bid, 100.0);
        assert_eq!(c.ask, 101.0);
    }

    #[test]
    fn parsea_rest_book_coinbase() {
        let msg = br#"{"bids":[["100.0","2.0",1]],"asks":[["101.0","1.5",1]],"time":"2024-03-09T00:00:00Z"}"#;
        let c = parsear_rest_coinbase(msg, "BTC/USD").unwrap();
        assert_eq!(c.par, "BTC/USD");
        assert_eq!(c.bid, 100.0);
    }

    #[test]
    fn parsea_rest_books_okx() {
        let msg = br#"{"code":"0","data":[{"bids":[["100.0","2.0","0","1"]],"asks":[["101.0","1.5","0","1"]],"ts":"1710000000000"}]}"#;
        let c = parsear_rest_okx(msg, "BTC/USD").unwrap();
        assert_eq!(c.bid, 100.0);
        assert_eq!(c.ask, 101.0);
    }

    #[test]
    fn parsea_rest_orderbook_bybit() {
        let msg = br#"{"retCode":0,"result":{"b":[["100.0","2.0"]],"a":[["101.0","1.5"]],"ts":"1710000000000"}}"#;
        let c = parsear_rest_bybit(msg, "BTC/USD").unwrap();
        assert_eq!(c.bid, 100.0);
        assert_eq!(c.ask, 101.0);
    }
}
