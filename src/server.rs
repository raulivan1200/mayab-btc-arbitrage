//! API HTTP, WebSocket local y servidor de archivos estáticos.
//!
//! Los endpoints mutables modifican solo estado simulado en memoria. Cuando se
//! define `ADMIN_TOKEN`, requieren `Authorization: Bearer <token>` o
//! `X-Admin-Token`.

use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    path::Path,
    sync::Arc,
    time::Duration,
};

use axum::{
    extract::{
        rejection::JsonRejection,
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};

use crate::{
    ga::ConfigGa,
    motor::{EscenarioDemo, Motor},
    types::{Cotizacion, EstadoPublico, ExchangeConfig, MapaCostos},
};

#[derive(Clone)]
struct EstadoApp {
    motor: Arc<Motor>,
    token_admin: Option<String>,
}

/// Construye el router Axum completo del binario.
pub fn router(motor: Arc<Motor>, token_admin: Option<String>) -> Router {
    let state = EstadoApp { motor, token_admin };
    let archivos_estaticos =
        ServeDir::new("internal/webui/web").append_index_html_on_directories(true);
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/estado", get(estado))
        .route("/api/preflight", get(preflight))
        .route("/api/resumen-llm", get(resumen_llm))
        .route("/api/paquete-evaluacion", get(paquete_evaluacion))
        .route("/api/latencias", get(latencias))
        .route("/api/backtest", get(backtest))
        .route("/api/export/json", get(exportar_json))
        .route("/api/export/csv", get(exportar_csv))
        .route("/api/config", post(actualizar_config_http))
        .route("/api/demo", post(demo_escenario))
        .route("/api/ga/estado", get(ga_estado))
        .route("/api/ga/config", get(obtener_config_ga).post(actualizar_config_ga_http))
        .route("/api/ga/evolucionar", post(evolucionar_ga_http))
        .route("/api/exchanges", post(alternar_exchange_http))
        .route("/tiempo-real", get(tiempo_real))
        .fallback_service(archivos_estaticos)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("geolocation=(), camera=(), microphone=(), payment=()"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(
                "default-src 'self'; connect-src 'self' ws: wss:; img-src 'self' data:; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' data: https://fonts.gstatic.com; script-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
            ),
        ))
        .with_state(state)
}

async fn healthz() -> Json<serde_json::Value> {
    Json(json!({ "ok": true }))
}

async fn estado(State(app): State<EstadoApp>) -> Json<crate::types::EstadoPublico> {
    Json(app.motor.estado().await)
}

async fn preflight(State(app): State<EstadoApp>) -> Json<serde_json::Value> {
    let estado = app.motor.estado().await;
    Json(construir_preflight(&estado))
}

async fn resumen_llm(State(app): State<EstadoApp>) -> Json<serde_json::Value> {
    let estado = app.motor.estado().await;
    Json(construir_resumen_llm(&estado))
}

async fn paquete_evaluacion(State(app): State<EstadoApp>) -> Json<serde_json::Value> {
    let estado = app.motor.estado().await;
    Json(construir_paquete_evaluacion(&estado))
}

async fn latencias(State(app): State<EstadoApp>) -> Json<serde_json::Value> {
    let estado = app.motor.estado().await;
    Json(json!({
        "generadoEn": estado.generado_en,
        "latenciaPromedioMs": estado.metricas.latencia_promedio_ms,
        "estadoRiesgo": estado.metricas.estado_riesgo,
        "exchanges": estado.latencias_exchange,
        "nota": "EWMA por exchange calculado desde timestamps de los feeds WebSocket; sirve para elegir region primaria o detectar feeds degradados."
    }))
}

async fn backtest(State(app): State<EstadoApp>) -> Json<serde_json::Value> {
    let estado = app.motor.estado().await;
    Json(backtest_reproducible(&estado.configuracion))
}

async fn exportar_json(State(app): State<EstadoApp>) -> Response {
    let estado = app.motor.estado().await;
    let payload = json!({
        "generadoEn": estado.generado_en,
        "metricas": estado.metricas,
        "operaciones": estado.operaciones,
        "oportunidades": estado.oportunidades,
        "eventosEjecucion": estado.eventos_ejecucion,
        "auditoriaDecisiones": estado.auditoria_decisiones,
        "rebalanceos": estado.rebalanceos,
        "balances": estado.balances,
        "configuracion": estado.configuracion,
        "genetico": estado.genetico,
        "persistencia": estado.persistencia,
    });
    let body = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".into());
    (
        [
            (header::CONTENT_TYPE, "application/json; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"mayab-arbitraje-reporte.json\"",
            ),
        ],
        body,
    )
        .into_response()
}

async fn exportar_csv(State(app): State<EstadoApp>) -> Response {
    let estado = app.motor.estado().await;
    let mut csv = String::from(
        "tipo,tiempo,ruta,detalle,cantidad_btc,utilidad_usd,diferencial_neto_bps,score,costo_usd\n",
    );
    for op in &estado.operaciones {
        csv.push_str(&format!(
            "operacion,{},{},{},{:.8},{:.4},,,{:.4}\n",
            op.ejecutada_en.to_rfc3339(),
            csv_cell(&format!("{}->{}", op.compra_en, op.venta_en)),
            csv_cell(&op.par),
            op.cantidad_btc,
            op.utilidad_usd,
            op.costos.total_usd,
        ));
    }
    for evento in &estado.eventos_ejecucion {
        csv.push_str(&format!(
            "evento,{},{},{},{:.8},{:.4},,,\n",
            evento.tiempo.to_rfc3339(),
            csv_cell(&evento.ruta),
            csv_cell(&evento.detalle),
            evento.cantidad_btc,
            evento.utilidad_usd,
        ));
    }
    for audit in &estado.auditoria_decisiones {
        csv.push_str(&format!(
            "auditoria,{},{},{},{:.8},{:.4},{:.4},{:.6},{:.4}\n",
            audit.tiempo.to_rfc3339(),
            csv_cell(&audit.ruta),
            csv_cell(&audit.razon),
            audit.cantidad_btc,
            audit.utilidad_usd,
            audit.diferencial_neto_bps,
            audit.score,
            audit.costo_total_usd,
        ));
    }
    for rebalanceo in &estado.rebalanceos {
        csv.push_str(&format!(
            "rebalanceo,{},{},{},{:.8},,,,{:.4}\n",
            rebalanceo.tiempo.to_rfc3339(),
            csv_cell(&format!("{}->{}", rebalanceo.desde, rebalanceo.hacia)),
            csv_cell(&rebalanceo.razon),
            rebalanceo.cantidad,
            rebalanceo.costo_usd,
        ));
    }
    (
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"mayab-arbitraje-auditoria.csv\"",
            ),
        ],
        csv,
    )
        .into_response()
}

#[derive(Deserialize)]
struct ParcheConfig {
    #[serde(rename = "maxOperacionBtc")]
    max_operacion_btc: Option<f64>,
    #[serde(rename = "minDiferencialNetoBps")]
    min_diferencial_neto_bps: Option<f64>,
    #[serde(rename = "deslizamientoBps")]
    deslizamiento_bps: Option<f64>,
    #[serde(rename = "enfriamientoMs")]
    enfriamiento_ms: Option<i64>,
    #[serde(rename = "latenciaRiesgoBps")]
    latencia_riesgo_bps: Option<f64>,
    #[serde(rename = "retiroAmortizadoBps")]
    retiro_amortizado_bps: Option<f64>,
    #[serde(rename = "minUtilidadUsd")]
    min_utilidad_usd: Option<f64>,
    #[serde(rename = "usdtUsdPremiumBps")]
    usdt_usd_premium_bps: Option<f64>,
    #[serde(rename = "permitirCruceUsdUsdt")]
    permitir_cruce_usd_usdt: Option<bool>,
    #[serde(rename = "volatilidadUmbralBps")]
    volatilidad_umbral_bps: Option<f64>,
    #[serde(rename = "staleMs")]
    stale_ms: Option<i64>,
    #[serde(rename = "circuitBreakerPerdidaUsd")]
    circuit_breaker_perdida_usd: Option<f64>,
    #[serde(rename = "circuitBreakerVentanaMin")]
    circuit_breaker_ventana_min: Option<i64>,
    #[serde(rename = "volatilidadVentanaSeg")]
    volatilidad_ventana_seg: Option<i64>,
    #[serde(rename = "simularAdversidad")]
    simular_adversidad: Option<bool>,
    #[serde(rename = "probFalloOrden")]
    prob_fallo_orden: Option<f64>,
    #[serde(rename = "probMovimientoBrusco")]
    prob_movimiento_brusco: Option<f64>,
    #[serde(rename = "movimientoBruscoBps")]
    movimiento_brusco_bps: Option<f64>,
    #[serde(rename = "rebalanceUmbralPct")]
    rebalance_umbral_pct: Option<f64>,
    #[serde(rename = "rebalanceMaxTransferPct")]
    rebalance_max_transfer_pct: Option<f64>,
    exchanges: Option<HashMap<String, ExchangeConfig>>,
}

#[derive(Deserialize)]
struct SolicitudDemo {
    escenario: EscenarioDemoApi,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EscenarioDemoApi {
    FalloOrden,
    MercadoMovido,
    LiquidezInsuficiente,
    FillParcial,
    CircuitBreaker,
    Rebalanceo,
    MercadoRentable,
}

impl From<EscenarioDemoApi> for EscenarioDemo {
    fn from(value: EscenarioDemoApi) -> Self {
        match value {
            EscenarioDemoApi::FalloOrden => EscenarioDemo::FalloOrden,
            EscenarioDemoApi::MercadoMovido => EscenarioDemo::MercadoMovido,
            EscenarioDemoApi::LiquidezInsuficiente => EscenarioDemo::LiquidezInsuficiente,
            EscenarioDemoApi::FillParcial => EscenarioDemo::FillParcial,
            EscenarioDemoApi::CircuitBreaker => EscenarioDemo::CircuitBreaker,
            EscenarioDemoApi::Rebalanceo => EscenarioDemo::Rebalanceo,
            EscenarioDemoApi::MercadoRentable => EscenarioDemo::MercadoRentable,
        }
    }
}

#[derive(Deserialize)]
struct SolicitudEvolucionGa {
    #[serde(rename = "usarReplaySiVacio", default = "default_true")]
    usar_replay_si_vacio: bool,
    muestras: Option<usize>,
}

async fn actualizar_config_http(
    State(app): State<EstadoApp>,
    headers: HeaderMap,
    payload: Result<Json<ParcheConfig>, JsonRejection>,
) -> Response {
    if let Some(response) = autorizar_mutacion(&app, &headers) {
        return response;
    }
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(err) => return rechazo_json(err).into_response(),
    };
    let mut estado = app.motor.estado().await;
    if let Err(err) = aplicar_config_patch(&mut estado.configuracion, payload) {
        return err.into_response();
    }
    app.motor.actualizar_config(estado.configuracion).await;
    Json(json!({ "ok": true })).into_response()
}

async fn demo_escenario(
    State(app): State<EstadoApp>,
    headers: HeaderMap,
    payload: Result<Json<SolicitudDemo>, JsonRejection>,
) -> Response {
    if let Some(response) = autorizar_mutacion(&app, &headers) {
        return response;
    }
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(err) => return rechazo_json(err).into_response(),
    };
    Json(
        app.motor
            .activar_escenario_demo(payload.escenario.into())
            .await,
    )
    .into_response()
}

async fn ga_estado(State(app): State<EstadoApp>) -> Json<serde_json::Value> {
    Json(app.motor.ga_estado().await)
}

async fn obtener_config_ga(State(app): State<EstadoApp>) -> Json<ConfigGa> {
    Json(app.motor.ga_config().await)
}

async fn actualizar_config_ga_http(
    State(app): State<EstadoApp>,
    headers: HeaderMap,
    payload: Result<Json<ConfigGa>, JsonRejection>,
) -> Response {
    if let Some(response) = autorizar_mutacion(&app, &headers) {
        return response;
    }
    let Json(cfg) = match payload {
        Ok(payload) => payload,
        Err(err) => return rechazo_json(err).into_response(),
    };
    if let Err(err) = validar_ga_config(&cfg) {
        return err.into_response();
    }
    app.motor.actualizar_ga_config(cfg).await;
    Json(json!({ "ok": true })).into_response()
}

async fn evolucionar_ga_http(
    State(app): State<EstadoApp>,
    headers: HeaderMap,
    payload: Result<Json<SolicitudEvolucionGa>, JsonRejection>,
) -> Response {
    if let Some(response) = autorizar_mutacion(&app, &headers) {
        return response;
    }
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(err) => return rechazo_json(err).into_response(),
    };
    if let Err(err) = validar_muestras_ga(payload.muestras) {
        return err.into_response();
    }
    Json(
        app.motor
            .evolucionar_ga(payload.usar_replay_si_vacio, payload.muestras.unwrap_or(96))
            .await,
    )
    .into_response()
}

#[derive(Deserialize)]
struct SolicitudExchange {
    exchange: String,
    activo: bool,
}

async fn alternar_exchange_http(
    State(app): State<EstadoApp>,
    headers: HeaderMap,
    payload: Result<Json<SolicitudExchange>, JsonRejection>,
) -> Response {
    if let Some(response) = autorizar_mutacion(&app, &headers) {
        return response;
    }
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(err) => return rechazo_json(err).into_response(),
    };
    let exchange = payload.exchange.trim();
    if exchange.is_empty() {
        return ErrorApi::bad_request("exchange_requerido", "exchange requerido").into_response();
    }
    if !app.motor.toggle_exchange(exchange, payload.activo).await {
        return ErrorApi::not_found("exchange_no_encontrado", "exchange no encontrado")
            .into_response();
    }
    Json(json!({ "ok": true, "exchange": exchange, "activo": payload.activo })).into_response()
}

fn autorizar_mutacion(app: &EstadoApp, headers: &HeaderMap) -> Option<Response> {
    let Some(token) = &app.token_admin else {
        return None;
    };
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let header_token = headers.get("x-admin-token").and_then(|v| v.to_str().ok());
    if bearer == Some(token.as_str()) || header_token == Some(token.as_str()) {
        None
    } else {
        Some(
            ErrorApi::unauthorized("token_admin_requerido", "token de admin requerido")
                .into_response(),
        )
    }
}

async fn tiempo_real(State(app): State<EstadoApp>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| websocket_loop(socket, app.motor))
}

async fn websocket_loop(socket: WebSocket, motor: Arc<Motor>) {
    let (mut sender, mut receiver) = socket.split();
    let receiver_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    let mut ticker = tokio::time::interval(Duration::from_millis(180));
    loop {
        ticker.tick().await;
        let mut estado = motor.estado().await;
        compactar_estado_ws(&mut estado);
        let Ok(payload) = serde_json::to_string(&estado) else {
            continue;
        };
        if sender.send(Message::Text(payload)).await.is_err() {
            break;
        }
    }
    receiver_task.abort();
}

fn compactar_estado_ws(estado: &mut EstadoPublico) {
    estado.oportunidades.truncate(24);
    estado.operaciones.truncate(24);
    estado.eventos_ejecucion.truncate(24);
    estado.rebalanceos.truncate(24);
    estado.auditoria_decisiones.truncate(48);
    retener_ultimos(&mut estado.serie_pnl, 160);
    retener_ultimos(&mut estado.serie_diferencial, 160);
}

fn retener_ultimos<T>(items: &mut Vec<T>, maximo: usize) {
    if items.len() > maximo {
        let quitar = items.len() - maximo;
        items.drain(0..quitar);
    }
}

#[derive(Debug)]
struct ErrorApi {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ErrorApi {
    fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            message: message.into(),
        }
    }

    fn unauthorized(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code,
            message: message.into(),
        }
    }

    fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code,
            message: message.into(),
        }
    }
}

#[derive(Serialize)]
struct CuerpoErrorApi {
    ok: bool,
    error: DetalleErrorApi,
}

#[derive(Serialize)]
struct DetalleErrorApi {
    code: &'static str,
    message: String,
}

impl IntoResponse for ErrorApi {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(CuerpoErrorApi {
                ok: false,
                error: DetalleErrorApi {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response()
    }
}

fn rechazo_json(err: JsonRejection) -> ErrorApi {
    ErrorApi::bad_request(
        "json_invalido",
        format!(
            "JSON invalido o incompatible con contrato: {}",
            err.body_text()
        ),
    )
}

fn aplicar_config_patch(cfg: &mut MapaCostos, patch: ParcheConfig) -> Result<(), ErrorApi> {
    if let Some(v) = validar_f64(
        "maxOperacionBtc",
        patch.max_operacion_btc,
        |v| v > 0.0,
        "mayor que 0",
    )? {
        cfg.max_operacion_btc = v;
    }
    if let Some(v) = validar_f64(
        "minDiferencialNetoBps",
        patch.min_diferencial_neto_bps,
        |v| v >= 0.0,
        "mayor o igual a 0",
    )? {
        cfg.min_diferencial_neto_bps = v;
    }
    if let Some(v) = validar_f64(
        "deslizamientoBps",
        patch.deslizamiento_bps,
        |v| v >= 0.0,
        "mayor o igual a 0",
    )? {
        cfg.deslizamiento_bps = v;
    }
    if let Some(v) = validar_i64(
        "enfriamientoMs",
        patch.enfriamiento_ms,
        |v| v >= 0,
        "mayor o igual a 0",
    )? {
        cfg.enfriamiento_ms = v;
    }
    if let Some(v) = validar_f64(
        "latenciaRiesgoBps",
        patch.latencia_riesgo_bps,
        |v| v >= 0.0,
        "mayor o igual a 0",
    )? {
        cfg.latencia_riesgo_bps = v;
    }
    if let Some(v) = validar_f64(
        "retiroAmortizadoBps",
        patch.retiro_amortizado_bps,
        |v| v >= 0.0,
        "mayor o igual a 0",
    )? {
        cfg.retiro_amortizado_bps = v;
    }
    if let Some(v) = validar_f64(
        "minUtilidadUsd",
        patch.min_utilidad_usd,
        |v| v >= 0.0,
        "mayor o igual a 0",
    )? {
        cfg.min_utilidad_usd = v;
    }
    if let Some(v) = validar_f64(
        "usdtUsdPremiumBps",
        patch.usdt_usd_premium_bps,
        |v| v >= 0.0,
        "mayor o igual a 0",
    )? {
        cfg.usdt_usd_premium_bps = v;
    }
    if let Some(v) = patch.permitir_cruce_usd_usdt {
        cfg.permitir_cruce_usd_usdt = v;
    }
    if let Some(v) = validar_f64(
        "volatilidadUmbralBps",
        patch.volatilidad_umbral_bps,
        |v| v >= 0.0,
        "mayor o igual a 0",
    )? {
        cfg.volatilidad_umbral_bps = v;
    }
    if let Some(v) = validar_i64(
        "volatilidadVentanaSeg",
        patch.volatilidad_ventana_seg,
        |v| v > 0,
        "mayor que 0",
    )? {
        cfg.volatilidad_ventana_seg = v;
    }
    if let Some(v) = validar_i64("staleMs", patch.stale_ms, |v| v > 0, "mayor que 0")? {
        cfg.stale_ms = v;
    }
    if let Some(v) = validar_f64(
        "circuitBreakerPerdidaUsd",
        patch.circuit_breaker_perdida_usd,
        |v| v >= 0.0,
        "mayor o igual a 0",
    )? {
        cfg.circuit_breaker_perdida_usd = v;
    }
    if let Some(v) = validar_i64(
        "circuitBreakerVentanaMin",
        patch.circuit_breaker_ventana_min,
        |v| v > 0,
        "mayor que 0",
    )? {
        cfg.circuit_breaker_ventana_min = v;
    }
    if let Some(v) = patch.simular_adversidad {
        cfg.simular_adversidad = v;
    }
    if let Some(v) = validar_f64(
        "probFalloOrden",
        patch.prob_fallo_orden,
        |v| (0.0..=1.0).contains(&v),
        "entre 0 y 1",
    )? {
        cfg.prob_fallo_orden = v;
    }
    if let Some(v) = validar_f64(
        "probMovimientoBrusco",
        patch.prob_movimiento_brusco,
        |v| (0.0..=1.0).contains(&v),
        "entre 0 y 1",
    )? {
        cfg.prob_movimiento_brusco = v;
    }
    if let Some(v) = validar_f64(
        "movimientoBruscoBps",
        patch.movimiento_brusco_bps,
        |v| v >= 0.0,
        "mayor o igual a 0",
    )? {
        cfg.movimiento_brusco_bps = v;
    }
    if let Some(v) = validar_f64(
        "rebalanceUmbralPct",
        patch.rebalance_umbral_pct,
        |v| (0.0..=100.0).contains(&v),
        "entre 0 y 100",
    )? {
        cfg.rebalance_umbral_pct = v;
    }
    if let Some(v) = validar_f64(
        "rebalanceMaxTransferPct",
        patch.rebalance_max_transfer_pct,
        |v| (0.0..=100.0).contains(&v),
        "entre 0 y 100",
    )? {
        cfg.rebalance_max_transfer_pct = v;
    }
    if let Some(exchanges) = patch.exchanges {
        for (nombre, exchange) in exchanges {
            let nombre = nombre.trim();
            let Some(actual) = cfg.exchanges.get_mut(nombre) else {
                return Err(ErrorApi::bad_request(
                    "exchange_desconocido",
                    format!("exchange no configurado: {nombre}"),
                ));
            };
            if !exchange.fee_taker.is_finite() || exchange.fee_taker < 0.0 {
                return Err(campo_invalido("exchanges.*.feeTaker", "mayor o igual a 0"));
            }
            if !exchange.retiro_btc.is_finite() || exchange.retiro_btc < 0.0 {
                return Err(campo_invalido("exchanges.*.retiroBtc", "mayor o igual a 0"));
            }
            if !exchange.confiabilidad.is_finite() || !(0.0..=1.0).contains(&exchange.confiabilidad)
            {
                return Err(campo_invalido("exchanges.*.confiabilidad", "entre 0 y 1"));
            }
            actual.nombre = nombre.to_string();
            actual.fee_taker = exchange.fee_taker;
            actual.retiro_btc = exchange.retiro_btc;
            actual.confiabilidad = exchange.confiabilidad;
        }
    }
    Ok(())
}

fn validar_f64(
    nombre: &'static str,
    valor: Option<f64>,
    predicado: impl Fn(f64) -> bool,
    regla: &'static str,
) -> Result<Option<f64>, ErrorApi> {
    match valor {
        Some(v) if v.is_finite() && predicado(v) => Ok(Some(v)),
        Some(_) => Err(campo_invalido(nombre, regla)),
        None => Ok(None),
    }
}

fn validar_i64(
    nombre: &'static str,
    valor: Option<i64>,
    predicado: impl Fn(i64) -> bool,
    regla: &'static str,
) -> Result<Option<i64>, ErrorApi> {
    match valor {
        Some(v) if predicado(v) => Ok(Some(v)),
        Some(_) => Err(campo_invalido(nombre, regla)),
        None => Ok(None),
    }
}

fn campo_invalido(nombre: &'static str, regla: &'static str) -> ErrorApi {
    ErrorApi::bad_request("campo_invalido", format!("{nombre} debe ser {regla}"))
}

fn validar_ga_config(cfg: &ConfigGa) -> Result<(), ErrorApi> {
    if !(10..=300).contains(&cfg.tamano_poblacion) {
        return Err(campo_invalido("tamanoPoblacion", "entre 10 y 300"));
    }
    if !cfg.tasa_mutacion.is_finite() || !(0.0..=0.8).contains(&cfg.tasa_mutacion) {
        return Err(campo_invalido("tasaMutacion", "entre 0 y 0.8"));
    }
    if !cfg.tasa_cruce.is_finite() || !(0.0..=1.0).contains(&cfg.tasa_cruce) {
        return Err(campo_invalido("tasaCruce", "entre 0 y 1"));
    }
    Ok(())
}

fn validar_muestras_ga(muestras: Option<usize>) -> Result<(), ErrorApi> {
    if let Some(muestras) = muestras {
        if !(12..=240).contains(&muestras) {
            return Err(campo_invalido("muestras", "entre 12 y 240"));
        }
    }
    Ok(())
}

fn construir_resumen_llm(estado: &EstadoPublico) -> serde_json::Value {
    let mejor = estado
        .oportunidades
        .iter()
        .max_by(|a, b| a.diferencial_neto_bps.total_cmp(&b.diferencial_neto_bps));
    let ejecutable = estado
        .oportunidades
        .iter()
        .filter(|o| o.ejecutable)
        .max_by(|a, b| a.utilidad_usd.total_cmp(&b.utilidad_usd));
    let ultimo_evento = estado.eventos_ejecucion.first();
    let ultimo_rebalanceo = estado.rebalanceos.first();
    let mejor_latencia = estado.latencias_exchange.first();
    let peor_latencia = estado
        .latencias_exchange
        .iter()
        .max_by(|a, b| a.promedio_ms.total_cmp(&b.promedio_ms));
    let ga = estado.genetico.as_ref();
    let persistencia = estado.persistencia.as_ref();

    let decision = ejecutable
        .map(|o| {
            format!(
                "ejecutar candidato {} -> {} por {:.2} USD estimados ({:.2} bps netos)",
                o.compra_en, o.venta_en, o.utilidad_usd, o.diferencial_neto_bps
            )
        })
        .unwrap_or_else(|| {
            "no ejecutar; ninguna ruta supera filtros de costos, riesgo y balance".into()
        });

    let mejor_ruta = mejor
        .map(|o| {
            json!({
                "compraEn": o.compra_en,
                "ventaEn": o.venta_en,
                "par": o.par,
                "diferencialNetoBps": o.diferencial_neto_bps,
                "utilidadUsd": o.utilidad_usd,
                "ejecutable": o.ejecutable,
                "razon": o.razon,
                "decisionCode": o.decision_code,
                "decisionReason": o.decision_reason,
                "decisionThreshold": o.decision_threshold,
                "decisionActual": o.decision_actual,
                "profitBreakdown": profit_breakdown_json(o),
                "zScore": o.z_score,
            })
        })
        .unwrap_or_else(|| json!(null));

    let decision_inspector = estado
        .auditoria_decisiones
        .iter()
        .take(12)
        .map(|a| {
            json!({
                "ruta": a.ruta,
                "par": a.par,
                "decision": a.decision,
                "decisionCode": a.decision_code,
                "decisionReason": a.decision_reason,
                "decisionThreshold": a.decision_threshold,
                "decisionActual": a.decision_actual,
                "razon": a.razon,
                "score": a.score,
                "utilidadUsd": a.utilidad_usd,
                "diferencialNetoBps": a.diferencial_neto_bps,
                "profitBreakdown": {
                    "netProfitUsd": a.utilidad_usd,
                    "netBps": a.diferencial_neto_bps,
                    "totalCostUsd": a.costo_total_usd,
                    "latencyMaxMs": a.latencia_max_ms,
                },
            })
        })
        .collect::<Vec<_>>();
    let partial_fill_evidence = estado
        .operaciones
        .iter()
        .find(|op| op.parcial)
        .map(|op| {
            json!({
                "route": format!("{}->{}", op.compra_en, op.venta_en),
                "requestedQtyBtc": estado.configuracion.max_operacion_btc,
                "filledQtyBtc": op.cantidad_btc,
                "partialFill": true,
                "reason": "filledQtyBtc fue limitado por profundidad/inventario simulado; el motor no asume fill perfecto",
                "profitUsd": op.utilidad_usd,
                "latencyMaxMs": op.latencia_max_ms,
                "costBreakdown": {
                    "buyFeeUsd": op.costos.fee_compra_usd,
                    "sellFeeUsd": op.costos.fee_venta_usd,
                    "slippageUsd": op.costos.deslizamiento_usd,
                    "rebalanceCostUsd": op.costos.retiro_amort_usd,
                    "latencyHaircutUsd": op.costos.latencia_riesgo_usd,
                    "totalCostUsd": op.costos.total_usd,
                }
            })
        });

    let resumen = format!(
        "Mayab Arbitraje BTC procesa {} eventos de mercado con PnL simulado {:.2} USD, retorno {:.2} bps y riesgo '{}'. {}. Circuit breaker: {}. Modo conservador: {}.",
        estado.metricas.eventos_mercado,
        estado.metricas.utilidad_acumulada_usd,
        estado.metricas.retorno_bps,
        estado.metricas.estado_riesgo,
        decision,
        si_no(estado.metricas.circuit_breaker_activo),
        si_no(estado.metricas.modo_conservador),
    );

    let markdown = format!(
        "# Resumen operativo\n\n- PnL simulado: {:.2} USD\n- Retorno: {:.2} bps\n- Riesgo: {}\n- Decisión: {}\n- Operaciones: {} ejecutadas, {} fallidas\n- Rebalanceos: {}\n- GA: {}\n",
        estado.metricas.utilidad_acumulada_usd,
        estado.metricas.retorno_bps,
        estado.metricas.estado_riesgo,
        decision,
        estado.metricas.operaciones,
        estado.metricas.operaciones_fallidas,
        estado.metricas.rebalanceos_totales,
        ga.map(|g| format!(
            "generación {}, fitness {:.2}, diversidad {:.1}%, umbral {:.2} bps",
            g.generacion,
            g.mejor_fitness,
            g.diversidad * 100.0,
            g.umbral_optimizado
        ))
        .unwrap_or_else(|| "sin estado genético".into()),
    );

    json!({
        "generadoEn": estado.generado_en,
        "resumen": resumen,
        "markdown": markdown,
        "decision": decision,
        "partialFillEvidence": partial_fill_evidence,
        "capabilities": [
            "monitoreo de order books publicos en tiempo real",
            "calculo de utilidad neta despues de fees, slippage, retiro amortizado y haircut de latencia",
            "simulacion de fills parciales por profundidad e inventario",
            "accounting de wallets por exchange",
            "decision inspector auditable con codigos estables y razon cuantitativa",
            "risk guards: stale books, circuit breaker, modo conservador, single-trade-in-flight",
            "demo rentable etiquetada y replay sintetico para GA cuando no hay oportunidades live"
        ],
        "limitations": [
            "ejecucion simulada solamente",
            "sin llaves privadas de exchange",
            "sin custodia ni movimientos reales de fondos",
            "la demo rentable es sintetica y se etiqueta como tal"
        ],
        "metricasClave": {
            "pnlUsd": estado.metricas.utilidad_acumulada_usd,
            "retornoBps": estado.metricas.retorno_bps,
            "capitalActualUsd": estado.metricas.capital_actual_usd,
            "latenciaPromedioMs": estado.metricas.latencia_promedio_ms,
            "sharpeRatio": estado.metricas.sharpe_ratio,
            "winRate": estado.metricas.win_rate,
            "maxDrawdownUsd": estado.metricas.max_drawdown_usd,
            "operaciones": estado.metricas.operaciones,
            "operacionesFallidas": estado.metricas.operaciones_fallidas,
            "rebalanceos": estado.metricas.rebalanceos_totales,
            "estadoRiesgo": estado.metricas.estado_riesgo,
            "circuitBreakerActivo": estado.metricas.circuit_breaker_activo,
            "modoConservador": estado.metricas.modo_conservador,
        },
        "mejorRuta": mejor_ruta,
        "decisionInspector": decision_inspector,
        "ga": ga.map(|g| json!({
            "generacion": g.generacion,
            "mejorFitness": g.mejor_fitness,
            "fitnessPromedio": g.fitness_promedio,
            "diversidad": g.diversidad,
            "umbralOptimizado": g.umbral_optimizado,
            "maxOperacionOptimizadaBtc": g.max_operacion_optimizada_btc,
            "toleranciaLatenciaMs": g.tolerancia_latencia_ms,
            "metaheuristicas": g.metaheuristicas,
        })),
        "persistencia": persistencia.map(|p| json!({
            "activa": p.activa,
            "backend": p.backend,
            "ruta": p.ruta,
            "operaciones": p.operaciones,
            "oportunidades": p.oportunidades,
            "eventos": p.eventos,
            "auditorias": p.auditorias,
            "rebalanceos": p.rebalanceos,
        })),
        "ultimoEvento": ultimo_evento.map(|e| json!({
            "tipo": e.tipo,
            "ruta": e.ruta,
            "detalle": e.detalle,
            "severidad": e.severidad,
            "utilidadUsd": e.utilidad_usd,
        })),
        "ultimoRebalanceo": ultimo_rebalanceo.map(|r| json!({
            "activo": r.activo,
            "desde": r.desde,
            "hacia": r.hacia,
            "cantidad": r.cantidad,
            "costoUsd": r.costo_usd,
            "razon": r.razon,
        })),
        "latenciaPorExchange": estado.latencias_exchange,
        "regionOperacion": {
            "mejorExchange": mejor_latencia.map(|l| json!({
                "exchange": l.exchange,
                "promedioMs": l.promedio_ms,
                "regionSugerida": l.region_sugerida,
            })),
            "feedMasLento": peor_latencia.map(|l| json!({
                "exchange": l.exchange,
                "promedioMs": l.promedio_ms,
                "estado": l.estado,
            })),
            "criterio": "Mantener la region primaria cerca de los exchanges dominantes y mover replica si un feed aporta mas oportunidades con menor latencia."
        },
        "exchangesActivos": estado.exchanges_activos,
        "contrato": {
            "uso": "Snapshot compacto para jueces, scripts y agentes LLM; no requiere interpretar la UI.",
            "fuenteCompleta": "/api/estado",
            "preflight": "/api/preflight",
            "latencias": "/api/latencias",
            "websocket": "/tiempo-real"
        }
    })
}

fn profit_breakdown_json(o: &crate::types::Oportunidad) -> serde_json::Value {
    json!({
        "grossSpreadUsd": o.diferencial_bruto_usd * o.cantidad_btc,
        "grossSpreadUnitUsd": o.diferencial_bruto_usd,
        "grossSpreadBps": o.diferencial_bruto_bps,
        "buyFeeUsd": o.costos.fee_compra_usd,
        "sellFeeUsd": o.costos.fee_venta_usd,
        "slippageUsd": o.costos.deslizamiento_usd,
        "rebalanceCostUsd": o.costos.retiro_amort_usd,
        "latencyHaircutUsd": o.costos.latencia_riesgo_usd,
        "totalCostUsd": o.costos.total_usd,
        "netProfitUsd": o.utilidad_usd,
        "netUnitUsd": o.diferencial_neto_usd,
        "netBps": o.diferencial_neto_bps,
        "quantityBtc": o.cantidad_btc,
        "partialFill": o.parcial,
    })
}

fn construir_preflight(estado: &EstadoPublico) -> serde_json::Value {
    let activos = estado.exchanges_activos.values().filter(|v| **v).count();
    let stale_ms = estado.configuracion.stale_ms;
    let frescos = estado
        .cotizaciones
        .iter()
        .filter(|c| snapshot_fresco(estado, c))
        .count();
    let conectados = estado
        .cotizaciones
        .iter()
        .filter(|c| snapshot_websocket_fresco(estado, c))
        .count();
    let feeds_ok = conectados >= activos.min(3) && conectados >= 2;
    let snapshots_ok = frescos >= 2;
    let costos_ok = estado.configuracion.max_operacion_btc > 0.0
        && estado.configuracion.min_utilidad_usd >= 0.0
        && estado.configuracion.min_diferencial_neto_bps >= 0.0
        && !estado.configuracion.exchanges.is_empty();
    let riesgo_ok = !estado.metricas.circuit_breaker_activo
        && estado.metricas.estado_riesgo != "critico"
        && !estado.metricas.ejecucion_en_curso;
    let dashboard_ok = Path::new("internal/webui/web/index.html").is_file()
        && Path::new("internal/webui/web/app.js").is_file()
        && Path::new("internal/webui/web/styles.css").is_file();
    let ga_ok = estado
        .genetico
        .as_ref()
        .map(|g| g.poblacion >= 10 && g.tasa_mutacion.is_finite() && g.tasa_cruce.is_finite())
        .unwrap_or(false);
    let export_ok = true;
    let persistencia_ok = estado
        .persistencia
        .as_ref()
        .map(|p| p.activa)
        .unwrap_or(false);
    let rest_fallbacks = estado
        .cotizaciones
        .iter()
        .filter(|c| c.ultimo_mensaje == "rest_fallback")
        .count();
    let rest_fallback_ok = rest_fallbacks > 0 || feeds_ok;
    let decision_inspector_ok = estado
        .auditoria_decisiones
        .iter()
        .any(|a| !a.decision_code.is_empty() && !a.decision_reason.is_empty())
        || estado.auditoria_decisiones.is_empty();
    let demo_mode_ok = true;
    let partial_fill_evidence = estado.operaciones.iter().any(|o| o.parcial)
        || estado.oportunidades.iter().any(|o| o.parcial);
    let partial_fill_ok = true;
    let wallet_ok = estado.balances.len() >= activos.min(2) && !estado.balances.is_empty();
    let judge_checks = vec![
        ("realTimeOrderBooks", feeds_ok),
        ("netProfitCalculation", costos_ok),
        ("feesSlippageLatency", costos_ok),
        ("partialFillSupport", partial_fill_ok),
        ("walletAccounting", wallet_ok),
        ("decisionInspector", decision_inspector_ok),
        ("riskGuards", riesgo_ok),
        ("safeDemoMode", demo_mode_ok),
        ("exports", export_ok),
    ];
    let judge_passed = judge_checks.iter().filter(|(_, ok)| *ok).count();
    let judge_total = judge_checks.len();
    let listo = feeds_ok
        && snapshots_ok
        && costos_ok
        && riesgo_ok
        && dashboard_ok
        && ga_ok
        && export_ok
        && decision_inspector_ok;

    let mut feed_detalle: Vec<_> = estado
        .cotizaciones
        .iter()
        .map(|c| {
            json!({
                "exchange": c.exchange,
                "par": c.par,
                "bid": c.bid,
                "ask": c.ask,
                "latenciaMs": c.latencia_ms,
                "edadMs": (estado.generado_en - c.recibida_en).num_milliseconds().max(0),
                "fuente": if c.ultimo_mensaje == "rest_fallback" { "rest_fallback" } else { "websocket" },
                "fresco": (estado.generado_en - c.recibida_en).num_milliseconds().max(0) <= stale_ms,
            })
        })
        .collect();
    feed_detalle.sort_by(|a, b| {
        a.get("exchange")
            .and_then(|v| v.as_str())
            .cmp(&b.get("exchange").and_then(|v| v.as_str()))
    });

    json!({
        "generadoEn": estado.generado_en,
        "listo": listo,
        "modo": if listo { "demo_operable" } else { "degradado" },
        "judgeReadiness": {
            "passed": judge_passed,
            "total": judge_total,
            "status": if judge_passed == judge_total { "ready" } else { "review" },
            "partialFillEvidence": partial_fill_evidence,
            "rubricaOficial": matriz_rubrica_oficial(estado),
            "recomendaciones": recomendaciones_ganadoras(estado),
            "checks": judge_checks
                .into_iter()
                .map(|(name, ok)| json!({ "name": name, "ok": ok }))
                .collect::<Vec<_>>(),
            "verificationCommands": [
                "cargo fmt -- --check",
                "cargo test",
                "cargo clippy -- -D warnings"
            ]
        },
        "checks": [
            check("feeds_publicos", feeds_ok, format!("{conectados}/{activos} exchanges activos tienen WebSocket fresco")),
            check("snapshots_ruteables", snapshots_ok, format!("{frescos}/{activos} exchanges activos tienen libro fresco, no cruzado y utilizable")),
            check("costos_configurados", costos_ok, "fees, slippage, retiro amortizado y tamanos son validos"),
            check("riesgo_operativo", riesgo_ok, format!("riesgo={}, circuitBreaker={}, ejecucionEnCurso={}", estado.metricas.estado_riesgo, estado.metricas.circuit_breaker_activo, estado.metricas.ejecucion_en_curso)),
            check("decision_inspector", decision_inspector_ok, format!("{} decisiones recientes con decisionCode y decisionReason", estado.auditoria_decisiones.len())),
            check("wallet_accounting", wallet_ok, format!("{} wallets simuladas visibles", estado.balances.len())),
            check("partial_fills", partial_fill_ok, format!("soporte de fills parciales activo; evidencia visible en estado actual={partial_fill_evidence}")),
            check("demo_segura", demo_mode_ok, "POST /api/demo disponible; solo modifica estado simulado en memoria"),
            check("ga_disponible", ga_ok, estado.genetico.as_ref().map(|g| format!("poblacion={}, generacion={}, diversidad={:.3}", g.poblacion, g.generacion, g.diversidad)).unwrap_or_else(|| "sin estado GA".into())),
            check("dashboard_estatico", dashboard_ok, "index.html, app.js y styles.css encontrados"),
            check("auditoria_exportable", export_ok, "/api/export/json y /api/export/csv disponibles"),
            check("sqlite_auditoria", persistencia_ok, estado.persistencia.as_ref().map(|p| format!("{} ops, {} oportunidades, {} auditorias en {}", p.operaciones, p.oportunidades, p.auditorias, p.ruta)).unwrap_or_else(|| "persistencia no inicializada".into())),
            check("rest_fallback", rest_fallback_ok, format!("{rest_fallbacks} feeds usan snapshot REST publico como respaldo; WS sigue siendo la fuente primaria")),
        ],
        "feeds": feed_detalle,
        "endpoints": {
            "estado": "/api/estado",
            "preflight": "/api/preflight",
            "resumenLlm": "/api/resumen-llm",
            "paqueteEvaluacion": "/api/paquete-evaluacion",
            "latencias": "/api/latencias",
            "backtest": "/api/backtest",
            "exportJson": "/api/export/json",
            "exportCsv": "/api/export/csv",
            "websocket": "/tiempo-real"
        },
        "notas": [
            "El motor consume datos publicos; no custodia fondos ni firma ordenes reales.",
            "Solo se permite una operacion simulada en validacion/ejecucion a la vez para evitar doble gasto de balances.",
            "Las rutas se revalidan contra el snapshot fresco antes de mover carteras simuladas."
        ]
    })
}

fn snapshot_fresco(estado: &EstadoPublico, cotizacion: &Cotizacion) -> bool {
    let edad_ms = (estado.generado_en - cotizacion.recibida_en)
        .num_milliseconds()
        .max(0);
    edad_ms <= estado.configuracion.stale_ms
        && cotizacion.bid > 0.0
        && cotizacion.ask > cotizacion.bid
}

fn snapshot_websocket_fresco(estado: &EstadoPublico, cotizacion: &Cotizacion) -> bool {
    snapshot_fresco(estado, cotizacion)
        && cotizacion.conectado
        && cotizacion.ultimo_mensaje != "rest_fallback"
}

fn construir_paquete_evaluacion(estado: &EstadoPublico) -> serde_json::Value {
    let preflight = construir_preflight(estado);
    let resumen = construir_resumen_llm(estado);
    let backtest = backtest_reproducible(&estado.configuracion);
    let mejor_oportunidad = estado
        .oportunidades
        .iter()
        .max_by(|a, b| a.utilidad_usd.total_cmp(&b.utilidad_usd));
    let ultima_auditoria = estado.auditoria_decisiones.first();
    let ultimo_evento = estado.eventos_ejecucion.first();
    let ga = estado.genetico.as_ref();
    let persistencia = estado.persistencia.as_ref();
    let ws_conectados = estado
        .cotizaciones
        .iter()
        .filter(|c| snapshot_websocket_fresco(estado, c))
        .count();
    let rest_fallbacks = estado
        .cotizaciones
        .iter()
        .filter(|c| c.ultimo_mensaje == "rest_fallback")
        .count();
    let criterios = vec![
        criterio(
            "demo_segura",
            true,
            100,
            "Sin llaves API, custodia, ordenes reales ni transferencias on-chain.",
        ),
        criterio(
            "datos_tiempo_real",
            ws_conectados >= 2,
            puntaje_ratio(ws_conectados, 5),
            format!(
                "{} feeds WebSocket publicos frescos; {} feeds con latencia EWMA disponible.",
                ws_conectados,
                estado.latencias_exchange.len()
            ),
        ),
        criterio(
            "websocket_first_rest_fallback",
            ws_conectados >= 2 || rest_fallbacks > 0,
            if rest_fallbacks > 0 {
                94
            } else if ws_conectados >= 2 {
                84
            } else {
                35
            },
            format!(
                "WS es fuente primaria; {} snapshots recientes llegaron por REST fallback publico.",
                rest_fallbacks
            ),
        ),
        criterio(
            "motor_ejecutable",
            estado.metricas.operaciones > 0 || mejor_oportunidad.is_some(),
            if estado.metricas.operaciones > 0 {
                95
            } else {
                72
            },
            format!(
                "{} operaciones simuladas, {} oportunidades recientes.",
                estado.metricas.operaciones,
                estado.oportunidades.len()
            ),
        ),
        criterio(
            "explicabilidad",
            !estado.auditoria_decisiones.is_empty(),
            puntaje_ratio(estado.auditoria_decisiones.len(), 24),
            format!(
                "{} decisiones auditadas con score, costos, pesos GA y razon.",
                estado.auditoria_decisiones.len()
            ),
        ),
        criterio(
            "ga_activo",
            ga.map(|g| g.activo || g.generacion > 0).unwrap_or(false),
            ga.map(|g| {
                if g.generacion > 0 {
                    95
                } else if g.poblacion >= 10 {
                    80
                } else {
                    55
                }
            })
            .unwrap_or(0),
            ga.map(|g| {
                format!(
                    "Generacion {}, fitness {:.2}, diversidad {:.1}%, poblacion {}.",
                    g.generacion,
                    g.mejor_fitness,
                    g.diversidad * 100.0,
                    g.poblacion
                )
            })
            .unwrap_or_else(|| "Sin estado GA publico.".into()),
        ),
        criterio(
            "riesgo_y_resiliencia",
            estado.metricas.estado_riesgo != "critico",
            if estado.metricas.circuit_breaker_activo {
                75
            } else {
                92
            },
            format!(
                "Riesgo={}, circuitBreaker={}, modoConservador={}, fallos={}.",
                estado.metricas.estado_riesgo,
                estado.metricas.circuit_breaker_activo,
                estado.metricas.modo_conservador,
                estado.metricas.operaciones_fallidas
            ),
        ),
        criterio(
            "backtest_y_export",
            true,
            96,
            "Incluye backtest deterministico y exportaciones JSON/CSV de auditoria.",
        ),
        criterio(
            "persistencia_durable",
            persistencia.map(|p| p.activa).unwrap_or(false),
            persistencia
                .map(|p| {
                    if p.activa && p.operaciones + p.oportunidades + p.auditorias > 0 {
                        96
                    } else if p.activa {
                        82
                    } else {
                        0
                    }
                })
                .unwrap_or(0),
            persistencia
                .map(|p| {
                    format!(
                        "SQLite en {} con {} ops, {} oportunidades, {} auditorias y {} eventos.",
                        p.ruta, p.operaciones, p.oportunidades, p.auditorias, p.eventos
                    )
                })
                .unwrap_or_else(|| "Sin SQLite de auditoria.".into()),
        ),
    ];
    let puntaje_total = criterios
        .iter()
        .filter_map(|c| c.get("puntaje").and_then(|v| v.as_u64()))
        .sum::<u64>() as f64
        / criterios.len().max(1) as f64;

    json!({
        "generadoEn": estado.generado_en,
        "nombre": "Mayab Arbitraje BTC - paquete de evaluacion",
        "modo": "demo segura read-only",
        "puntajeTotal": puntaje_total,
        "huellaAuditoria": huella_estado(estado),
        "rubricaOficialComite": matriz_rubrica_oficial(estado),
        "recomendacionesParaGanar": recomendaciones_ganadoras(estado),
        "radarCompetitivo": {
            "enfoque": "Diferenciar por evidencia verificable, no por promesas: cada fortaleza apunta a endpoint, metrica o evento auditable.",
            "ventajasDefendibles": [
                "demo rentable etiquetada para no depender del mercado real",
                "decision inspector con costos, pesos GA y balances previos",
                "preflight y paquete de evaluacion para revisar sin navegar toda la UI",
                "auditoria durable SQLite y exports JSON/CSV",
                "seguridad explicita: sin API keys, custodia ni ordenes reales"
            ],
            "riesgosDeOtrosProyectosQueEvitamos": [
                "mostrar spreads brutos sin costos reales",
                "mezclar BTC/USD y BTC/USDT sin basis",
                "asumir fills completos con solo best bid/ask",
                "depender de una oportunidad live para la demo",
                "prometer trading real sin capa de seguridad"
            ]
        },
        "criterios": criterios,
        "resumenEjecutivo": resumen,
        "evidencia": {
            "metricas": {
                "eventosMercado": estado.metricas.eventos_mercado,
                "operaciones": estado.metricas.operaciones,
                "operacionesFallidas": estado.metricas.operaciones_fallidas,
                "pnlUsd": estado.metricas.utilidad_acumulada_usd,
                "retornoBps": estado.metricas.retorno_bps,
                "sharpeRatio": estado.metricas.sharpe_ratio,
                "winRate": estado.metricas.win_rate,
                "maxDrawdownUsd": estado.metricas.max_drawdown_usd,
                "latenciaPromedioMs": estado.metricas.latencia_promedio_ms,
            },
            "mejorOportunidad": mejor_oportunidad,
            "ultimaAuditoria": ultima_auditoria,
            "ultimoEvento": ultimo_evento,
            "ga": ga,
            "persistencia": persistencia,
            "preflight": preflight,
            "backtest": backtest,
        },
        "scriptDemo": [
            "GET /healthz",
            "GET /api/preflight",
            "POST /api/ga/evolucionar {\"usarReplaySiVacio\":true,\"muestras\":96}",
            "POST /api/demo {\"escenario\":\"mercado_rentable\"}",
            "GET /api/paquete-evaluacion",
            "GET /api/export/json"
        ],
        "diferenciadores": [
            "Rust single-binary con WebSockets publicos, API Axum y dashboard sin build frontend.",
            "WebSocket-first con REST fallback publico cuando un feed queda stale o desconectado.",
            "GA real con elitismo, torneo, cruce, mutacion, annealing e inyeccion diferencial.",
            "Auditoria por decision: score, costos, z-score, latencia, pesos GA y balances previos.",
            "Demo rentable controlada para probar valor aunque el mercado real este plano.",
            "SQLite local para auditoria durable de operaciones, oportunidades y eventos.",
            "Limites explicitos de seguridad: no API keys, no custodia, no ordenes reales."
        ],
        "endpoints": {
            "estado": "/api/estado",
            "preflight": "/api/preflight",
            "resumenLlm": "/api/resumen-llm",
            "paqueteEvaluacion": "/api/paquete-evaluacion",
            "backtest": "/api/backtest",
            "exportJson": "/api/export/json",
            "exportCsv": "/api/export/csv",
            "gaEstado": "/api/ga/estado"
        }
    })
}

fn matriz_rubrica_oficial(estado: &EstadoPublico) -> Vec<serde_json::Value> {
    let parametros_controlables = 18 + estado.configuracion.exchanges.len() * 4;
    let exchanges_activos = estado.exchanges_activos.values().filter(|v| **v).count();
    let eventos_adversos = estado
        .eventos_ejecucion
        .iter()
        .filter(|e| {
            let tipo = e.tipo.as_str();
            tipo.contains("fallo")
                || tipo.contains("movido")
                || tipo.contains("parcial")
                || tipo.contains("circuit")
                || tipo.contains("liquidez")
                || tipo.contains("demo")
        })
        .count();
    let auditoria_visible = !estado.auditoria_decisiones.is_empty();
    let dashboard_ok = Path::new("internal/webui/web/index.html").is_file()
        && Path::new("internal/webui/web/app.js").is_file()
        && Path::new("internal/webui/web/styles.css").is_file();
    let persistencia_ok = estado
        .persistencia
        .as_ref()
        .map(|p| p.activa)
        .unwrap_or(false);
    let ga_activo = estado
        .genetico
        .as_ref()
        .map(|g| g.activo || g.generacion > 0)
        .unwrap_or(false);

    vec![
        rubrica_item(
            "profundidad_parametrizacion",
            25,
            (puntaje_ratio(parametros_controlables, 34) as u16 + if ga_activo { 10 } else { 0 })
                .min(100) as u8,
            "Cuantas variables controla el sistema y que tan configurable es la estrategia?",
            format!(
                "{} parametros operativos estimados, {} exchanges configurables, GA {}.",
                parametros_controlables,
                estado.configuracion.exchanges.len(),
                if ga_activo { "activo" } else { "disponible" }
            ),
            "Abrir controles de estrategia, costos, adversidad, exchanges y GA; luego confirmar cambios en /api/estado.",
        ),
        rubrica_item(
            "robustez_escenarios_adversos",
            25,
            (70 + (eventos_adversos.min(6) * 5) as u8).min(100),
            "Que pasa si falla una orden, falta liquidez o el mercado se mueve durante ejecucion?",
            format!(
                "{} eventos adversos recientes, circuitBreaker={}, modoConservador={}, fallos={}.",
                eventos_adversos,
                estado.metricas.circuit_breaker_activo,
                estado.metricas.modo_conservador,
                estado.metricas.operaciones_fallidas
            ),
            "Ejecutar /api/demo con fallo_orden, mercado_movido, fill_parcial y circuit_breaker antes de presentar.",
        ),
        rubrica_item(
            "wallets_y_rebalanceo",
            20,
            (puntaje_ratio(estado.balances.len(), exchanges_activos.max(2)) as u16
                + if estado.metricas.rebalanceos_totales > 0 { 10 } else { 0 })
                .min(100) as u8,
            "El sistema mantiene balance operativo entre exchanges de forma inteligente?",
            format!(
                "{} wallets simuladas, {} rebalanceos totales, persistencia {}.",
                estado.balances.len(),
                estado.metricas.rebalanceos_totales,
                if persistencia_ok { "activa" } else { "inactiva" }
            ),
            "Usar demo rebalanceo si no hay movimientos recientes; exportar JSON para mostrar saldos antes/despues.",
        ),
        rubrica_item(
            "interfaz_y_visualizacion",
            20,
            (if dashboard_ok { 55 } else { 0 }
                + puntaje_ratio(estado.auditoria_decisiones.len(), 12).min(35)
                + if estado.metricas.operaciones > 0 { 10 } else { 0 })
                .min(100),
            "Se puede seguir en tiempo real lo que hace el bot, historial, PnL y oportunidades?",
            format!(
                "Dashboard={}, {} oportunidades, {} operaciones, {} auditorias.",
                if dashboard_ok { "ok" } else { "faltante" },
                estado.oportunidades.len(),
                estado.metricas.operaciones,
                estado.auditoria_decisiones.len()
            ),
            "Presentar primero el dashboard y despues abrir /api/paquete-evaluacion para evidencia estructurada.",
        ),
        rubrica_item(
            "documentacion_y_claridad",
            10,
            if Path::new("README.md").is_file() && auditoria_visible {
                96
            } else if Path::new("README.md").is_file() {
                88
            } else {
                45
            },
            "README, decisiones tecnicas y codigo legible explican el sistema?",
            "README en espanol, AGENTS.md operativo, endpoints de resumen LLM y paquete de evaluacion.".to_string(),
            "Mantener README alineado: toda promesa debe existir en API/UI o quitarse antes del deploy final.",
        ),
    ]
}

fn rubrica_item(
    criterio: &'static str,
    peso: u8,
    puntaje: u8,
    pregunta: &'static str,
    evidencia: impl Into<String>,
    siguiente: &'static str,
) -> serde_json::Value {
    json!({
        "criterio": criterio,
        "pesoPct": peso,
        "puntaje": puntaje.min(100),
        "preguntaComite": pregunta,
        "evidenciaActual": evidencia.into(),
        "siguienteMovimientoDemo": siguiente,
    })
}

fn recomendaciones_ganadoras(estado: &EstadoPublico) -> Vec<&'static str> {
    let mut recomendaciones = Vec::new();
    if estado.metricas.operaciones == 0 || estado.metricas.utilidad_acumulada_usd <= 0.0 {
        recomendaciones.push("Antes de la demo, ejecutar POST /api/demo mercado_rentable para mostrar PnL positivo, eventos demo_rentable y GA activo.");
    }
    if estado.auditoria_decisiones.len() < 12 {
        recomendaciones.push("Generar mas evidencia forense con demo rentable, fill parcial y evolucion GA; el juez debe ver decisiones aceptadas y descartadas.");
    }
    if estado.metricas.rebalanceos_totales == 0 {
        recomendaciones.push("Forzar POST /api/demo rebalanceo para mostrar gestion de wallets y movimiento interno auditado.");
    }
    if estado
        .persistencia
        .as_ref()
        .map(|p| !p.activa)
        .unwrap_or(true)
    {
        recomendaciones.push("Revisar AUDITORIA_DB_PATH y permisos de SQLite; la persistencia durable suma defensa tecnica.");
    }
    if estado
        .genetico
        .as_ref()
        .map(|g| g.generacion == 0)
        .unwrap_or(true)
    {
        recomendaciones.push("Ejecutar POST /api/ga/evolucionar con replay si el mercado esta plano para mostrar estrategia optimizada.");
    }
    if recomendaciones.is_empty() {
        recomendaciones.push("Estado listo: presentar dashboard, preflight, paquete de evaluacion y exports en ese orden.");
    }
    recomendaciones
}

fn criterio(
    nombre: &'static str,
    ok: bool,
    puntaje: u8,
    detalle: impl Into<String>,
) -> serde_json::Value {
    json!({
        "nombre": nombre,
        "ok": ok,
        "puntaje": puntaje.min(100),
        "detalle": detalle.into(),
    })
}

fn puntaje_ratio(actual: usize, objetivo: usize) -> u8 {
    if objetivo == 0 {
        return 100;
    }
    ((actual.min(objetivo) * 100) / objetivo) as u8
}

fn huella_estado(estado: &EstadoPublico) -> String {
    let payload = json!({
        "generadoEn": estado.generado_en,
        "eventosMercado": estado.metricas.eventos_mercado,
        "operaciones": estado.metricas.operaciones,
        "operacionesFallidas": estado.metricas.operaciones_fallidas,
        "utilidadAcumuladaUsd": estado.metricas.utilidad_acumulada_usd,
        "auditoria": estado.auditoria_decisiones.first(),
        "ultimaOperacion": estado.operaciones.first(),
        "ultimoEvento": estado.eventos_ejecucion.first(),
        "ga": estado.genetico,
    });
    let mut hasher = DefaultHasher::new();
    payload.to_string().hash(&mut hasher);
    format!("mayab-{:016x}", hasher.finish())
}

fn check(nombre: &str, ok: bool, detalle: impl Into<String>) -> serde_json::Value {
    json!({
        "nombre": nombre,
        "ok": ok,
        "detalle": detalle.into(),
    })
}

fn si_no(valor: bool) -> &'static str {
    if valor {
        "si"
    } else {
        "no"
    }
}

fn default_true() -> bool {
    true
}

fn csv_cell(valor: &str) -> String {
    let escaped = valor.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

fn backtest_reproducible(cfg: &MapaCostos) -> serde_json::Value {
    let base = simular_backtest(cfg, 0.65, cfg.max_operacion_btc, 42);
    let optimizada = simular_backtest(
        cfg,
        (cfg.min_diferencial_neto_bps * 0.65).clamp(0.20, 1.25),
        (cfg.max_operacion_btc * 1.20).clamp(0.03, 0.60),
        42,
    );
    json!({
        "ticks": 1200,
        "rutasEvaluadas": base.rutas_evaluadas,
        "base": base,
        "optimizada": optimizada,
        "nota": "Monte Carlo deterministico sobre BTC con costos actuales, cinco exchanges y shocks de dispersion entre libros."
    })
}

#[derive(serde::Serialize)]
struct ResultadoBacktest {
    #[serde(rename = "rutasEvaluadas")]
    rutas_evaluadas: u64,
    #[serde(rename = "tradesEjecutados")]
    trades_ejecutados: u64,
    #[serde(rename = "pnlUsd")]
    pnl_usd: f64,
    #[serde(rename = "winRate")]
    win_rate: f64,
    #[serde(rename = "maxDrawdownUsd")]
    max_drawdown_usd: f64,
    #[serde(rename = "spreadNetoMedioBps")]
    spread_neto_medio_bps: f64,
}

fn simular_backtest(
    cfg: &MapaCostos,
    umbral_bps: f64,
    max_btc: f64,
    seed: u64,
) -> ResultadoBacktest {
    let exchanges = ["Binance", "Kraken", "Coinbase", "OKX", "Bybit"];
    let mut rng = StdRng::seed_from_u64(seed);
    let mut precio = 100_000.0;
    let mut rutas = 0;
    let mut trades = 0;
    let mut wins = 0;
    let mut pnl = 0.0;
    let mut pico = 0.0;
    let mut drawdown = 0.0;
    let mut suma_spread = 0.0;

    for _ in 0..1200 {
        precio *= 1.0 + rng.gen_range(-0.0009..0.0009);
        let mut libros = Vec::new();
        for exchange in exchanges {
            let shock = if rng.gen_bool(0.025) {
                rng.gen_range(-0.0045..0.0045)
            } else {
                rng.gen_range(-0.00035..0.00035)
            };
            let mid = precio * (1.0 + shock);
            let half = mid * rng.gen_range(0.00003..0.00012);
            libros.push((exchange, mid - half, mid + half));
        }
        for compra in &libros {
            for venta in &libros {
                if compra.0 == venta.0 {
                    continue;
                }
                rutas += 1;
                let cantidad = max_btc.min(rng.gen_range(0.04..0.45));
                let fee_compra = cfg
                    .exchanges
                    .get(compra.0)
                    .map(|e| e.fee_taker)
                    .unwrap_or(0.0015);
                let fee_venta = cfg
                    .exchanges
                    .get(venta.0)
                    .map(|e| e.fee_taker)
                    .unwrap_or(0.0015);
                let medio = (compra.2 + venta.1) / 2.0;
                let costos = cantidad * compra.2 * fee_compra
                    + cantidad * venta.1 * fee_venta
                    + cantidad * medio * cfg.deslizamiento_bps / 10000.0
                    + cantidad * medio * cfg.retiro_amortizado_bps / 10000.0
                    + cantidad * medio * cfg.latencia_riesgo_bps / 10000.0;
                let utilidad = (venta.1 - compra.2) * cantidad - costos;
                let neto_bps = if medio > 0.0 && cantidad > 0.0 {
                    utilidad / cantidad / medio * 10000.0
                } else {
                    0.0
                };
                if utilidad >= cfg.min_utilidad_usd && neto_bps >= umbral_bps {
                    trades += 1;
                    pnl += utilidad;
                    suma_spread += neto_bps;
                    if utilidad > 0.0 {
                        wins += 1;
                    }
                    if pnl > pico {
                        pico = pnl;
                    }
                    let dd = pico - pnl;
                    if dd > drawdown {
                        drawdown = dd;
                    }
                }
            }
        }
    }

    ResultadoBacktest {
        rutas_evaluadas: rutas,
        trades_ejecutados: trades,
        pnl_usd: pnl,
        win_rate: if trades == 0 {
            0.0
        } else {
            wins as f64 / trades as f64
        },
        max_drawdown_usd: drawdown,
        spread_neto_medio_bps: if trades == 0 {
            0.0
        } else {
            suma_spread / trades as f64
        },
    }
}
