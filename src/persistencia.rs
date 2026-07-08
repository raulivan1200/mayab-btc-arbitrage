//! Auditoría durable local en SQLite.
//!
//! La persistencia es deliberadamente local y sin credenciales: guarda eventos
//! simulados para auditoría y revisión posterior, sin tocar exchanges ni fondos.

use std::{
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex, MutexGuard,
    },
    time::Duration,
};

use anyhow::{anyhow, Context};
use rusqlite::{params, Connection};

use crate::types::{
    AuditoriaDecision, EstadoPersistencia, EventoEjecucion, Operacion, Oportunidad, Rebalanceo,
};

pub struct Persistencia {
    ruta: String,
    conn: Mutex<Connection>,
    operaciones: AtomicUsize,
    oportunidades: AtomicUsize,
    eventos: AtomicUsize,
    auditorias: AtomicUsize,
    rebalanceos: AtomicUsize,
}

impl Persistencia {
    pub fn abrir(ruta: &str) -> anyhow::Result<Self> {
        let path = Path::new(ruta);
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("no se pudo crear directorio SQLite {}", parent.display())
            })?;
        }
        let conn =
            Connection::open(path).with_context(|| format!("no se pudo abrir SQLite {ruta}"))?;
        conn.busy_timeout(Duration::from_secs(2))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        inicializar_schema(&conn)?;
        let operaciones = contar_tabla(&conn, "operaciones")?;
        let oportunidades = contar_tabla(&conn, "oportunidades")?;
        let eventos = contar_tabla(&conn, "eventos")?;
        let auditorias = contar_tabla(&conn, "auditorias")?;
        let rebalanceos = contar_tabla(&conn, "rebalanceos")?;
        Ok(Self {
            ruta: ruta.to_string(),
            conn: Mutex::new(conn),
            operaciones: AtomicUsize::new(operaciones),
            oportunidades: AtomicUsize::new(oportunidades),
            eventos: AtomicUsize::new(eventos),
            auditorias: AtomicUsize::new(auditorias),
            rebalanceos: AtomicUsize::new(rebalanceos),
        })
    }

    pub fn estado(&self) -> EstadoPersistencia {
        EstadoPersistencia {
            activa: true,
            backend: "sqlite".to_string(),
            ruta: self.ruta.clone(),
            operaciones: self.operaciones.load(Ordering::Relaxed),
            oportunidades: self.oportunidades.load(Ordering::Relaxed),
            eventos: self.eventos.load(Ordering::Relaxed),
            auditorias: self.auditorias.load(Ordering::Relaxed),
            rebalanceos: self.rebalanceos.load(Ordering::Relaxed),
            error: None,
        }
    }

    pub fn registrar_operacion(&self, op: &Operacion) -> anyhow::Result<()> {
        let conn = self.conn()?;
        let changed = conn.execute(
            "INSERT OR IGNORE INTO operaciones
             (id, tiempo, ruta, par, cantidad_btc, utilidad_usd, parcial, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                op.id,
                op.ejecutada_en.to_rfc3339(),
                format!("{}->{}", op.compra_en, op.venta_en),
                op.par,
                decimal_string(op.cantidad_btc, 8),
                decimal_string(op.utilidad_usd, 6),
                op.parcial,
                serde_json::to_string(op)?,
            ],
        )?;
        if changed > 0 {
            self.operaciones.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }

    pub fn registrar_evento(&self, evento: &EventoEjecucion) -> anyhow::Result<()> {
        let conn = self.conn()?;
        let changed = conn.execute(
            "INSERT OR IGNORE INTO eventos
             (id, tiempo, tipo, ruta, severidad, utilidad_usd, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                evento.id,
                evento.tiempo.to_rfc3339(),
                evento.tipo,
                evento.ruta,
                evento.severidad,
                decimal_string(evento.utilidad_usd, 6),
                serde_json::to_string(evento)?,
            ],
        )?;
        if changed > 0 {
            self.eventos.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }

    pub fn registrar_rebalanceo(&self, rebalanceo: &Rebalanceo) -> anyhow::Result<()> {
        let conn = self.conn()?;
        let changed = conn.execute(
            "INSERT OR IGNORE INTO rebalanceos
             (id, tiempo, ruta, activo, cantidad, costo_usd, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                rebalanceo.id,
                rebalanceo.tiempo.to_rfc3339(),
                format!("{}->{}", rebalanceo.desde, rebalanceo.hacia),
                rebalanceo.activo,
                decimal_string(rebalanceo.cantidad, 8),
                decimal_string(rebalanceo.costo_usd, 6),
                serde_json::to_string(rebalanceo)?,
            ],
        )?;
        if changed > 0 {
            self.rebalanceos.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }

    pub fn registrar_oportunidades(&self, oportunidades: &[Oportunidad]) -> anyhow::Result<()> {
        if oportunidades.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let changed = {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO oportunidades
                 (id, tiempo, ruta, par, ejecutable, utilidad_usd, diferencial_neto_bps, payload_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            let mut changed = 0usize;
            for op in oportunidades {
                changed += stmt.execute(params![
                    op.id,
                    op.detectada_en.to_rfc3339(),
                    format!("{}->{}", op.compra_en, op.venta_en),
                    op.par,
                    op.ejecutable,
                    decimal_string(op.utilidad_usd, 6),
                    decimal_string(op.diferencial_neto_bps, 6),
                    serde_json::to_string(op)?,
                ])?;
            }
            changed
        };
        tx.commit()?;
        if changed > 0 {
            self.oportunidades.fetch_add(changed, Ordering::Relaxed);
        }
        Ok(())
    }

    pub fn registrar_auditorias(&self, auditorias: &[AuditoriaDecision]) -> anyhow::Result<()> {
        if auditorias.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let changed = {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO auditorias
                 (id, tiempo, ruta, decision, score, utilidad_usd, razon, payload_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            let mut changed = 0usize;
            for audit in auditorias {
                changed += stmt.execute(params![
                    audit.id,
                    audit.tiempo.to_rfc3339(),
                    audit.ruta,
                    audit.decision,
                    decimal_string(audit.score, 8),
                    decimal_string(audit.utilidad_usd, 6),
                    audit.razon,
                    serde_json::to_string(audit)?,
                ])?;
            }
            changed
        };
        tx.commit()?;
        if changed > 0 {
            self.auditorias.fetch_add(changed, Ordering::Relaxed);
        }
        Ok(())
    }

    fn conn(&self) -> anyhow::Result<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| anyhow!("conexion SQLite bloqueada por panic previo"))
    }
}

fn contar_tabla(conn: &Connection, tabla: &'static str) -> anyhow::Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {tabla}");
    let total: i64 = conn.query_row(&sql, [], |row| row.get(0))?;
    Ok(total.max(0) as usize)
}

fn inicializar_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS operaciones (
            id TEXT PRIMARY KEY,
            tiempo TEXT NOT NULL,
            ruta TEXT NOT NULL,
            par TEXT NOT NULL,
            cantidad_btc TEXT NOT NULL,
            utilidad_usd TEXT NOT NULL,
            parcial INTEGER NOT NULL,
            payload_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS oportunidades (
            id TEXT PRIMARY KEY,
            tiempo TEXT NOT NULL,
            ruta TEXT NOT NULL,
            par TEXT NOT NULL,
            ejecutable INTEGER NOT NULL,
            utilidad_usd TEXT NOT NULL,
            diferencial_neto_bps TEXT NOT NULL,
            payload_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS eventos (
            id TEXT PRIMARY KEY,
            tiempo TEXT NOT NULL,
            tipo TEXT NOT NULL,
            ruta TEXT NOT NULL,
            severidad TEXT NOT NULL,
            utilidad_usd TEXT NOT NULL,
            payload_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS auditorias (
            id TEXT PRIMARY KEY,
            tiempo TEXT NOT NULL,
            ruta TEXT NOT NULL,
            decision TEXT NOT NULL,
            score TEXT NOT NULL,
            utilidad_usd TEXT NOT NULL,
            razon TEXT NOT NULL,
            payload_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS rebalanceos (
            id TEXT PRIMARY KEY,
            tiempo TEXT NOT NULL,
            ruta TEXT NOT NULL,
            activo TEXT NOT NULL,
            cantidad TEXT NOT NULL,
            costo_usd TEXT NOT NULL,
            payload_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_operaciones_tiempo ON operaciones(tiempo DESC);
        CREATE INDEX IF NOT EXISTS idx_oportunidades_tiempo ON oportunidades(tiempo DESC);
        CREATE INDEX IF NOT EXISTS idx_eventos_tiempo ON eventos(tiempo DESC);
        CREATE INDEX IF NOT EXISTS idx_auditorias_tiempo ON auditorias(tiempo DESC);
        CREATE INDEX IF NOT EXISTS idx_rebalanceos_tiempo ON rebalanceos(tiempo DESC);
        "#,
    )?;
    Ok(())
}

fn decimal_string(valor: f64, decimales: usize) -> String {
    if valor.is_finite() {
        format!("{valor:.decimales$}")
    } else {
        "0".to_string()
    }
}
