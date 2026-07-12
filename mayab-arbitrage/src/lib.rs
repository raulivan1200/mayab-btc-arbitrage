//! Mayab Arbitraje BTC - Librería core para tests de integración.

pub mod auditoria;
pub mod config;
pub mod discord;
pub mod estrategia;
pub mod evaluation;
pub mod ga;
pub mod impacto;
pub mod mercado;
pub mod metricas;
pub mod motor;
pub mod persistencia;
#[cfg(feature = "timescaledb")]
pub mod persistencia_timescale;
pub mod server;
pub mod tape;
#[cfg(feature = "testnet-execution")]
pub mod testnet;
pub mod types;
