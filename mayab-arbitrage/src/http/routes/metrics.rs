use crate::server::{self, EstadoApp};
use axum::{routing::get, Router};

pub(crate) fn routes() -> Router<EstadoApp> {
    Router::new()
        .route("/metrics", get(server::metrics))
        .route("/api/metrics", get(server::metrics))
        .route("/api/paquete-evaluacion", get(server::paquete_evaluacion))
}
