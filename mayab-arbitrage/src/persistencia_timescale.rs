//! Backend de auditoría TimescaleDB/Postgres (opt-in, feature `timescaledb`).
//!
//! Implementa el mismo contrato [`crate::auditoria::Auditoria`] que la
//! persistencia SQLite local, pero sobre hypertables de TimescaleDB. Se activa
//! con `cargo build --features timescaledb` y requiere `DATABASE_URL` apuntando
//! a una instancia TimescaleDB con el esquema de `scripts/timescaledb/schema.sql`.
//!
//! El motor no cambia: basta intercambiar la implementación de `Auditoria`.

use anyhow::Context;
use serde_json::json;
use tokio::sync::Mutex;
use tokio_postgres::{Client, NoTls, Row};

use crate::auditoria::Auditoria;
use crate::types::{
    AuditoriaDecision, EstadoPersistencia, EventoEjecucion, Operacion, Oportunidad, Rebalanceo,
};

pub struct TimescaleDbAuditoria {
    cliente: Mutex<Client>,
    url: String,
}

impl TimescaleDbAuditoria {
    /// Conecta a TimescaleDB y verifica el esquema.
    pub async fn abrir(url: &str) -> anyhow::Result<Self> {
        let (cliente, conexion) = tokio_postgres::connect(url, NoTls)
            .await
            .with_context(|| format!("no se pudo conectar a TimescaleDB {url}"))?;
        tokio::spawn(async move {
            if let Err(err) = conexion.await {
                tracing::warn!(error = %err, "conexion TimescaleDB cerrada");
            }
        });
        cliente
            .batch_execute("SELECT 1 FROM operaciones LIMIT 1; SELECT 1 FROM auditorias LIMIT 1;")
            .await
            .context("el esquema TimescaleDB no está inicializado")?;
        Ok(Self {
            cliente: Mutex::new(cliente),
            url: url.to_string(),
        })
    }

    fn rt(&self) -> anyhow::Result<tokio::runtime::Handle> {
        tokio::runtime::Handle::try_current().context("el registro requiere un runtime Tokio")
    }
}

async fn contar_tabla(c: &Client, tabla: &str) -> i64 {
    c.query_one(&format!("SELECT COUNT(*) FROM {tabla}"), &[])
        .await
        .ok()
        .map(|r| r.get::<_, i64>(0))
        .unwrap_or(0)
}

fn fila_operacion(fila: &Row) -> Operacion {
    let raw: String = fila.get("payload_json");
    serde_json::from_str(&raw).expect("payload de operacion siempre es valido")
}

impl Auditoria for TimescaleDbAuditoria {
    fn registrar_operacion(&self, op: &Operacion) -> anyhow::Result<()> {
        let rt = self.rt()?;
        let payload = json!(op).to_string();
        let (id, compra, venta, par, cantidad, utilidad, costo, parcial) = (
            op.id.clone(),
            op.compra_en.clone(),
            op.venta_en.clone(),
            op.par.clone(),
            op.cantidad_btc,
            op.utilidad_usd,
            op.costos.total_usd,
            op.parcial,
        );
        rt.block_on(async move {
            let c = self.cliente.lock().await;
            c.execute(
                "INSERT INTO operaciones (tiempo, id, compra_en, venta_en, par, cantidad_btc, utilidad_usd, costo_usd, score, partial_fill, payload_json) VALUES (NOW(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10::jsonb)",
                &[&id, &compra, &venta, &par, &cantidad, &utilidad, &costo, &None::<f64>, &parcial, &payload],
            )
            .await?;
            Ok::<_, anyhow::Error>(())
        })
    }

    fn registrar_evento(&self, evento: &EventoEjecucion) -> anyhow::Result<()> {
        let rt = self.rt()?;
        let payload = json!(evento).to_string();
        let (id, tipo, severidad, detalle) = (
            evento.id.clone(),
            evento.tipo.clone(),
            evento.severidad.clone(),
            evento.detalle.clone(),
        );
        rt.block_on(async move {
            let c = self.cliente.lock().await;
            c.execute(
                "INSERT INTO eventos (tiempo, id, tipo, severidad, mensaje, payload_json) VALUES (NOW(), $1, $2, $3, $4, $5::jsonb)",
                &[&id, &tipo, &severidad, &detalle, &payload],
            )
            .await?;
            Ok::<_, anyhow::Error>(())
        })
    }

    fn registrar_rebalanceo(&self, r: &Rebalanceo) -> anyhow::Result<()> {
        let rt = self.rt()?;
        let payload = json!(r).to_string();
        let (id, desde, hacia, cantidad, costo) = (
            r.id.clone(),
            r.desde.clone(),
            r.hacia.clone(),
            r.cantidad,
            r.costo_usd,
        );
        rt.block_on(async move {
            let c = self.cliente.lock().await;
            c.execute(
                "INSERT INTO rebalanceos (tiempo, id, desde, hacia, cantidad, costo_usd, payload_json) VALUES (NOW(), $1, $2, $3, $4, $5, $6::jsonb)",
                &[&id, &desde, &hacia, &cantidad, &costo, &payload],
            )
            .await?;
            Ok::<_, anyhow::Error>(())
        })
    }

    fn registrar_oportunidades(&self, oportunidades: &[Oportunidad]) -> anyhow::Result<()> {
        for o in oportunidades {
            let rt = self.rt()?;
            let payload = json!(o).to_string();
            let (id, compra, venta, utilidad, diff, payload) = (
                o.id.clone(),
                o.compra_en.clone(),
                o.venta_en.clone(),
                o.utilidad_usd,
                o.diferencial_neto_bps,
                payload,
            );
            rt.block_on(async move {
                let c = self.cliente.lock().await;
                c.execute(
                    "INSERT INTO oportunidades (tiempo, id, ruta, utilidad_usd, diferencial, payload_json) VALUES (NOW(), $1, $2, $3, $4, $5::jsonb)",
                    &[&id, &format!("{compra}->{venta}"), &utilidad, &diff, &payload],
                )
                .await?;
                Ok::<_, anyhow::Error>(())
            })?;
        }
        Ok(())
    }

    fn registrar_auditorias(&self, auditorias: &[AuditoriaDecision]) -> anyhow::Result<()> {
        for a in auditorias {
            let rt = self.rt()?;
            let payload = json!(a).to_string();
            let (id, ruta, decision, score, utilidad, razon, payload) = (
                a.id.clone(),
                a.ruta.clone(),
                a.decision_code.clone(),
                a.score,
                a.utilidad_usd,
                a.decision_reason.clone(),
                payload,
            );
            rt.block_on(async move {
                let c = self.cliente.lock().await;
                c.execute(
                    "INSERT INTO auditorias (tiempo, id, ruta, decision, score, utilidad_usd, razon, payload_json) VALUES (NOW(), $1, $2, $3, $4, $5, $6, $7::jsonb)",
                    &[&id, &ruta, &decision, &score, &utilidad, &razon, &payload],
                )
                .await?;
                Ok::<_, anyhow::Error>(())
            })?;
        }
        Ok(())
    }

    fn estado(&self) -> EstadoPersistencia {
        let rt = match self.rt() {
            Ok(rt) => rt,
            Err(_) => return EstadoPersistencia::inactiva(&self.url),
        };
        rt.block_on(async {
            let c = self.cliente.lock().await;
            let operaciones = contar_tabla(&c, "operaciones").await;
            let oportunidades = contar_tabla(&c, "oportunidades").await;
            let auditorias = contar_tabla(&c, "auditorias").await;
            let eventos = contar_tabla(&c, "eventos").await;
            let rebalanceos = contar_tabla(&c, "rebalanceos").await;
            EstadoPersistencia {
                activa: true,
                backend: "timescaledb".to_string(),
                ruta: self.url.clone(),
                operaciones: operaciones.max(0) as usize,
                oportunidades: oportunidades.max(0) as usize,
                auditorias: auditorias.max(0) as usize,
                eventos: eventos.max(0) as usize,
                rebalanceos: rebalanceos.max(0) as usize,
                db_bytes: 0,
                error: None,
            }
        })
    }

    fn total_pnl(&self) -> f64 {
        let rt = match self.rt() {
            Ok(rt) => rt,
            Err(_) => return 0.0,
        };
        rt.block_on(async {
            let c = self.cliente.lock().await;
            c.query_one(
                "SELECT COALESCE(SUM(utilidad_usd), 0.0) FROM operaciones",
                &[],
            )
            .await
            .ok()
            .map(|r| r.get::<_, f64>(0))
            .unwrap_or(0.0)
        })
    }

    fn win_rate(&self) -> f64 {
        let rt = match self.rt() {
            Ok(rt) => rt,
            Err(_) => return 0.0,
        };
        rt.block_on(async {
            let c = self.cliente.lock().await;
            let total: i64 = c
                .query_one("SELECT COUNT(*) FROM operaciones", &[])
                .await
                .ok()
                .map(|r| r.get::<_, i64>(0))
                .unwrap_or(0);
            if total == 0 {
                return 0.0;
            }
            let ganadas: i64 = c
                .query_one(
                    "SELECT COUNT(*) FROM operaciones WHERE utilidad_usd > 0",
                    &[],
                )
                .await
                .ok()
                .map(|r| r.get::<_, i64>(0))
                .unwrap_or(0);
            ganadas as f64 / total as f64 * 100.0
        })
    }

    fn ultimas_operaciones(&self, limite: usize) -> Vec<Operacion> {
        let rt = match self.rt() {
            Ok(rt) => rt,
            Err(_) => return Vec::new(),
        };
        rt.block_on(async {
            let c = self.cliente.lock().await;
            c.query(
                "SELECT payload_json::text AS payload_json FROM operaciones ORDER BY tiempo DESC LIMIT $1",
                &[&(limite as i64)],
            )
            .await
            .ok()
            .map(|filas| filas.iter().map(fila_operacion).collect::<Vec<_>>())
            .unwrap_or_default()
        })
    }

    fn resumen_agregado(&self) -> serde_json::Value {
        json!({
            "backend": "timescaledb",
            "totalPnl": self.total_pnl(),
            "winRate": self.win_rate(),
            "ruta": self.url,
        })
    }
}
