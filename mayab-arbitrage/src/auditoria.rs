//! Abstracción de auditoría durable (repository pattern).
//!
//! El motor no depende de SQLite: usa `Arc<dyn Auditoria>`. La implementación
//! por defecto es [`crate::persistencia::Persistencia`] (SQLite local). Una
//! implementación TimescaleDB/Postgres puede sustituirla tras habilitar la
//! feature `timescaledb`, sin tocar el motor ni la API.

use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    mpsc::{self, SyncSender, TrySendError},
    Arc,
};

use anyhow::{anyhow, Result};

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

enum EscrituraAuditoria {
    Operacion(Operacion),
    Evento(EventoEjecucion),
    Rebalanceo(Rebalanceo),
    Oportunidades(Vec<Oportunidad>),
    Auditorias(Vec<AuditoriaDecision>),
}

/// Worker único de persistencia con backpressure acotado y no bloqueante.
pub struct AuditoriaEnCola {
    backend: Arc<dyn Auditoria>,
    tx: SyncSender<EscrituraAuditoria>,
    pendientes: Arc<AtomicUsize>,
    descartadas: AtomicU64,
    capacidad: usize,
}

impl AuditoriaEnCola {
    pub fn nueva(backend: Arc<dyn Auditoria>, capacidad: usize) -> Self {
        let capacidad = capacidad.max(1);
        let (tx, rx) = mpsc::sync_channel(capacidad);
        let pendientes = Arc::new(AtomicUsize::new(0));
        let pendientes_worker = pendientes.clone();
        let backend_worker = backend.clone();
        std::thread::Builder::new()
            .name("mayab-persistence".to_string())
            .spawn(move || {
                while let Ok(escritura) = rx.recv() {
                    let resultado = match escritura {
                        EscrituraAuditoria::Operacion(v) => backend_worker.registrar_operacion(&v),
                        EscrituraAuditoria::Evento(v) => backend_worker.registrar_evento(&v),
                        EscrituraAuditoria::Rebalanceo(v) => {
                            backend_worker.registrar_rebalanceo(&v)
                        }
                        EscrituraAuditoria::Oportunidades(v) => {
                            backend_worker.registrar_oportunidades(&v)
                        }
                        EscrituraAuditoria::Auditorias(v) => {
                            backend_worker.registrar_auditorias(&v)
                        }
                    };
                    pendientes_worker.fetch_sub(1, Ordering::Relaxed);
                    if let Err(error) = resultado {
                        tracing::warn!(%error, "fallo del worker de persistencia");
                    }
                }
            })
            .expect("no se pudo iniciar worker de persistencia");
        Self {
            backend,
            tx,
            pendientes,
            descartadas: AtomicU64::new(0),
            capacidad,
        }
    }

    fn encolar(&self, escritura: EscrituraAuditoria) -> Result<()> {
        self.pendientes.fetch_add(1, Ordering::Relaxed);
        match self.tx.try_send(escritura) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                self.pendientes.fetch_sub(1, Ordering::Relaxed);
                self.descartadas.fetch_add(1, Ordering::Relaxed);
                Err(anyhow!("cola de persistencia llena"))
            }
            Err(TrySendError::Disconnected(_)) => {
                self.pendientes.fetch_sub(1, Ordering::Relaxed);
                Err(anyhow!("worker de persistencia detenido"))
            }
        }
    }
}

impl Auditoria for AuditoriaEnCola {
    fn registrar_operacion(&self, v: &Operacion) -> Result<()> {
        self.encolar(EscrituraAuditoria::Operacion(v.clone()))
    }
    fn registrar_evento(&self, v: &EventoEjecucion) -> Result<()> {
        self.encolar(EscrituraAuditoria::Evento(v.clone()))
    }
    fn registrar_rebalanceo(&self, v: &Rebalanceo) -> Result<()> {
        self.encolar(EscrituraAuditoria::Rebalanceo(v.clone()))
    }
    fn registrar_oportunidades(&self, v: &[Oportunidad]) -> Result<()> {
        self.encolar(EscrituraAuditoria::Oportunidades(v.to_vec()))
    }
    fn registrar_auditorias(&self, v: &[AuditoriaDecision]) -> Result<()> {
        self.encolar(EscrituraAuditoria::Auditorias(v.to_vec()))
    }
    fn estado(&self) -> EstadoPersistencia {
        let mut estado = self.backend.estado();
        estado.queue_capacity = self.capacidad;
        estado.queue_pending = self.pendientes.load(Ordering::Relaxed);
        estado.queue_dropped = self.descartadas.load(Ordering::Relaxed);
        estado
    }
    fn total_pnl(&self) -> f64 {
        self.backend.total_pnl()
    }
    fn win_rate(&self) -> f64 {
        self.backend.win_rate()
    }
    fn ultimas_operaciones(&self, limite: usize) -> Vec<Operacion> {
        self.backend.ultimas_operaciones(limite)
    }
    fn resumen_agregado(&self) -> serde_json::Value {
        self.backend.resumen_agregado()
    }
}
