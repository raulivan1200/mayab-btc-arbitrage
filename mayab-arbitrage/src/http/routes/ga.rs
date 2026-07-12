use crate::server::{self, EstadoApp};
use axum::{
    routing::{get, post},
    Router,
};

pub(crate) fn routes() -> Router<EstadoApp> {
    Router::new()
        .route("/api/ga/estado", get(server::ga_estado))
        .route("/api/ga/evolucionar", post(server::evolucionar_ga_http))
        .route(
            "/api/ga/config",
            get(server::obtener_config_ga).post(server::actualizar_config_ga_http),
        )
        .route("/api/ga/sensibilidad", get(server::ga_sensibilidad))
        .route("/api/ga/ablacion", get(server::ga_sensibilidad))
}
