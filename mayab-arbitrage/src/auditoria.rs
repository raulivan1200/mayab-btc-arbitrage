//! Abstracción de auditoría durable (repository pattern).
//!
//! El motor no depende de SQLite: usa `Arc<dyn Auditoria>`. La implementación
//! por defecto es [`crate::persistencia::Persistencia`] (SQLite local). Una
//! implementación TimescaleDB/Postgres puede sustituirla tras habilitar la
//! feature `timescaledb`, sin tocar el motor ni la API.

use anyhow::Result;

use crate::types::{
    AuditoriaDecision, EstadoPersistencia, EventoEjecucion, Operacion, Oportunidad, Rebalanceo,
};

/// Contrato de persistencia de auditoría para el motor.
pub trait Auditoria: Send + Sync {
    /// Registra una operación simulada.
    fn registrar_operacion(&self, op: &Operacion) -> Result<()>;
    /// Registra un evento de ejecución.
    fn registrar_evento(&self, evento: &EventoEjecucion) -> Result<()>;
    /// Registra un rebalanceo de carteras.
    fn registrar_rebalanceo(&self, rebalanceo: &Rebalanceo) -> Result<()>;
    /// Registra oportunidades detectadas.
    fn registrar_oportunidades(&self, oportunidades: &[Oportunidad]) -> Result<()>;
    /// Registra decisiones auditadas.
    fn registrar_auditorias(&self, auditorias: &[AuditoriaDecision]) -> Result<()>;
    /// Snapshot de estado de la capa de persistencia.
    fn estado(&self) -> EstadoPersistencia;
    /// PnL total acumulado en la auditoría.
    fn total_pnl(&self) -> f64;
    /// Win rate agregado.
    fn win_rate(&self) -> f64;
    /// Últimas operaciones registradas.
    fn ultimas_operaciones(&self, limite: usize) -> Vec<Operacion>;
    /// Resumen agregado para el contrato público.
    fn resumen_agregado(&self) -> serde_json::Value;
}
