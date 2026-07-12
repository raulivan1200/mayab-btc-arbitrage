use crate::server::{self, EstadoApp};
use axum::{routing::get, Router};

pub(crate) fn routes() -> Router<EstadoApp> {
    Router::new()
        .route("/api/export/json", get(server::exportar_json))
        .route("/api/export/csv", get(server::exportar_csv))
        .route("/api/export/evidence", get(server::exportar_evidence))
}
