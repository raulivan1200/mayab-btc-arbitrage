use axum::{
    extract::State,
    response::{IntoResponse, Json},
};
use serde_json::json;

use crate::motor::Motor;

#[derive(Clone)]
pub struct JuryRouteState {
    pub motor: std::sync::Arc<Motor>,
}

pub async fn jurado(State(state): State<JuryRouteState>) -> Json<serde_json::Value> {
    let estado = state.motor.estado().await;
    Json(crate::server::construir_modo_jurado(&estado))
}

pub async fn paquete_evaluacion(State(state): State<JuryRouteState>) -> Json<serde_json::Value> {
    let estado = state.motor.estado().await;
    Json(crate::server::construir_paquete_evaluacion(&estado))
}
