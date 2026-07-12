use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPatchDto {
    #[serde(rename = "maxOperacionBtc")]
    pub max_operacion_btc: Option<f64>,
    #[serde(rename = "minDiferencialNetoBps")]
    pub min_diferencial_neto_bps: Option<f64>,
    #[serde(rename = "deslizamientoBps")]
    pub deslizamiento_bps: Option<f64>,
    #[serde(rename = "enfriamientoMs")]
    pub enfriamiento_ms: Option<i64>,
    #[serde(rename = "latenciaRiesgoBps")]
    pub latencia_riesgo_bps: Option<f64>,
    #[serde(rename = "retiroAmortizadoBps")]
    pub retiro_amortizado_bps: Option<f64>,
    #[serde(rename = "minUtilidadUsd")]
    pub min_utilidad_usd: Option<f64>,
    #[serde(rename = "usdtUsdPremiumBps")]
    pub usdt_usd_premium_bps: Option<f64>,
    #[serde(rename = "permitirCruceUsdUsdt")]
    pub permitir_cruce_usd_usdt: Option<bool>,
    #[serde(rename = "volatilidadUmbralBps")]
    pub volatilidad_umbral_bps: Option<f64>,
    #[serde(rename = "staleMs")]
    pub stale_ms: Option<i64>,
    #[serde(rename = "circuitBreakerPerdidaUsd")]
    pub circuit_breaker_perdida_usd: Option<f64>,
    #[serde(rename = "circuitBreakerVentanaMin")]
    pub circuit_breaker_ventana_min: Option<i64>,
    #[serde(rename = "volatilidadVentanaSeg")]
    pub volatilidad_ventana_seg: Option<i64>,
    #[serde(rename = "simularAdversidad")]
    pub simular_adversidad: Option<bool>,
    #[serde(rename = "probFalloOrden")]
    pub prob_fallo_orden: Option<f64>,
    #[serde(rename = "probMovimientoBrusco")]
    pub prob_movimiento_brusco: Option<f64>,
    #[serde(rename = "movimientoBruscoBps")]
    pub movimiento_brusco_bps: Option<f64>,
    #[serde(rename = "rebalanceUmbralPct")]
    pub rebalance_umbral_pct: Option<f64>,
    #[serde(rename = "rebalanceMaxTransferPct")]
    pub rebalance_max_transfer_pct: Option<f64>,
    #[serde(rename = "costoRebalanceoUsd")]
    pub costo_rebalanceo_usd: Option<f64>,
    #[serde(rename = "rebalanceSettlementMs")]
    pub rebalance_settlement_ms: Option<i64>,
    #[serde(rename = "webhookUrl")]
    pub webhook_url: Option<String>,
    pub exchanges: Option<std::collections::HashMap<String, ExchangeConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DemoScenarioDto {
    FalloOrden,
    FalloSegundaPierna,
    MercadoMovido,
    LiquidezInsuficiente,
    FillParcial,
    CircuitBreaker,
    Rebalanceo,
    MercadoRentable,
}

impl From<DemoScenarioDto> for crate::motor::EscenarioDemo {
    fn from(v: DemoScenarioDto) -> Self {
        match v {
            DemoScenarioDto::FalloOrden => Self::FalloOrden,
            DemoScenarioDto::FalloSegundaPierna => Self::FalloSegundaPierna,
            DemoScenarioDto::MercadoMovido => Self::MercadoMovido,
            DemoScenarioDto::LiquidezInsuficiente => Self::LiquidezInsuficiente,
            DemoScenarioDto::FillParcial => Self::FillParcial,
            DemoScenarioDto::CircuitBreaker => Self::CircuitBreaker,
            DemoScenarioDto::Rebalanceo => Self::Rebalanceo,
            DemoScenarioDto::MercadoRentable => Self::MercadoRentable,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoRequestDto {
    pub escenario: DemoScenarioDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaEvolveRequestDto {
    #[serde(rename = "usarReplaySiVacio", default = "default_true")]
    pub usar_replay_si_vacio: bool,
    pub muestras: Option<usize>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaConfigDto {
    #[serde(rename = "tamanoPoblacion")]
    pub tamano_poblacion: usize,
    #[serde(rename = "tasaMutacion")]
    pub tasa_mutacion: f64,
    #[serde(rename = "tasaCruce")]
    pub tasa_cruce: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeToggleDto {
    pub exchange: String,
    pub activo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceRulesDto {
    pub reglas: Vec<ReglaRebalanceo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCallDto {
    pub tool: String,
    #[serde(default)]
    pub arguments: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_scenario_deserialization() {
        let json = r#"{"escenario":"mercado_rentable"}"#;
        let req: DemoRequestDto = serde_json::from_str(json).unwrap();
        assert!(matches!(req.escenario, DemoScenarioDto::MercadoRentable));
    }

    #[test]
    fn test_ga_evolve_deserialization() {
        let json = r#"{"usarReplaySiVacio":true,"muestras":96}"#;
        let req: GaEvolveRequestDto = serde_json::from_str(json).unwrap();
        assert!(req.usar_replay_si_vacio);
        assert_eq!(req.muestras, Some(96));

        let json = r#"{}"#;
        let req: GaEvolveRequestDto = serde_json::from_str(json).unwrap();
        assert!(req.usar_replay_si_vacio);
        assert_eq!(req.muestras, None);
    }
}
