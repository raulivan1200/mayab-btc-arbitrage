use crate::server::{self, EstadoApp};
use axum::{
    routing::{get, post},
    Router,
};

pub(crate) fn routes() -> Router<EstadoApp> {
    Router::new()
        .route("/api/exchanges", post(server::alternar_exchange_http))
        .route(
            "/api/rebalance/rules",
            post(server::actualizar_reglas_rebalanceo_http),
        )
        .route("/api/admin/kill-switch", post(server::kill_switch_http))
        .route("/api/admin/config", post(server::actualizar_config_http))
        .route(
            "/api/admin/ga/config",
            post(server::actualizar_config_ga_http),
        )
        .route(
            "/api/admin/ga/evolucionar",
            post(server::evolucionar_ga_http),
        )
        .route("/api/admin/adverso", post(server::trigger_adverso_http))
        .route(
            "/api/admin/captura/iniciar",
            post(server::captura_iniciar_http),
        )
        .route(
            "/api/admin/captura/detener",
            post(server::captura_detener_http),
        )
        .route(
            "/api/admin/captura/replay",
            post(server::captura_replay_http),
        )
        .route(
            "/api/admin/captura/estado",
            get(server::captura_estado_http),
        )
}
