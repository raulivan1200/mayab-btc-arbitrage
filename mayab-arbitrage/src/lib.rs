//! Mayab Arbitraje BTC - Librería core para tests de integración.

pub mod auditoria;
pub mod config;
pub mod estrategia;
pub mod ga;
pub mod mercado;
pub mod metricas;
pub mod motor;
pub mod persistencia;
#[cfg(feature = "timescaledb")]
pub mod persistencia_timescale;
pub mod server;
pub mod types;
