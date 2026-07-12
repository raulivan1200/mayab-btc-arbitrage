use crate::server::{self, EstadoApp};
use axum::{routing::get, Router};

pub(crate) fn routes() -> Router<EstadoApp> {
    Router::new().route("/tiempo-real", get(server::tiempo_real))
}
