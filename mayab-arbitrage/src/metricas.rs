//! Métricas de observabilidad en formato de exposición Prometheus.
//!
//! Se evita una dependencia externa y se renderiza texto plano compatible con
//! scrapers de Prometheus. Los contadores de HTTP se actualizan vía middleware
//! y las métricas de motor se proyectan desde `EstadoPublico` en cada scrape.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use crate::types::EstadoPublico;

#[derive(Clone, Default)]
pub struct Metricas {
    inner: Arc<MetricasInner>,
}

#[derive(Default)]
struct MetricasInner {
    http_requests_total: Mutex<HashMap<(String, String, u16), u64>>,
    http_request_ms_sum: Mutex<HashMap<String, f64>>,
    http_request_ms_count: Mutex<HashMap<String, u64>>,
}

impl Metricas {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn registrar_peticion(
        &self,
        ruta: &str,
        metodo: &str,
        status: u16,
        duracion: std::time::Duration,
    ) {
        let mut map = self.inner.http_requests_total.lock().unwrap();
        *map.entry((metodo.to_string(), ruta.to_string(), status))
            .or_insert(0) += 1;
        let ms = duracion.as_secs_f64() * 1000.0;
        *self
            .inner
            .http_request_ms_sum
            .lock()
            .unwrap()
            .entry(ruta.to_string())
            .or_insert(0.0) += ms;
        *self
            .inner
            .http_request_ms_count
            .lock()
            .unwrap()
            .entry(ruta.to_string())
            .or_insert(0) += 1;
    }

    /// Renderiza todas las métricas en formato de texto Prometheus.
    pub fn render(&self, estado: &EstadoPublico) -> String {
        let mut out = String::new();
        let mut emit = |name: &str, help: &str, metric_type: &str, body: &str| {
            out.push_str(&format!("# HELP {name} {help}\n"));
            out.push_str(&format!("# TYPE {name} {metric_type}\n"));
            out.push_str(body);
        };

        // Contadores de HTTP por ruta/método/status.
        {
            let map = self.inner.http_requests_total.lock().unwrap();
            let mut body = String::new();
            for ((metodo, ruta, status), n) in map.iter() {
                body.push_str(&format!(
                    "mayab_http_requests_total{{metodo=\"{metodo}\",ruta=\"{ruta}\",status=\"{status}\"}} {n}\n"
                ));
            }
            emit(
                "mayab_http_requests_total",
                "Total de peticiones HTTP por ruta, metodo y status.",
                "counter",
                &body,
            );
        }

        // Latencia de HTTP (suma y conteo; el histograma se deriva en consulta).
        {
            let sum = self.inner.http_request_ms_sum.lock().unwrap();
            let count = self.inner.http_request_ms_count.lock().unwrap();
            let mut body = String::new();
            for (ruta, s) in sum.iter() {
                body.push_str(&format!(
                    "mayab_http_request_ms_sum{{ruta=\"{ruta}\"}} {s:.3}\n"
                ));
            }
            for (ruta, c) in count.iter() {
                body.push_str(&format!(
                    "mayab_http_request_ms_count{{ruta=\"{ruta}\"}} {c}\n"
                ));
            }
            emit(
                "mayab_http_request_ms",
                "Suma y conteo de latencia de peticiones HTTP en milisegundos.",
                "summary",
                &body,
            );
        }

        // Gauges proyectados desde el estado público.
        let m = &estado.metricas;
        let exchanges_activos = estado.exchanges_activos.values().filter(|v| **v).count();
        let ws_conectados = estado.cotizaciones.iter().filter(|c| c.conectado).count();
        let circuit = if m.circuit_breaker_activo { 1 } else { 0 };
        let mut gauges = String::new();
        gauges.push_str(&format!("mayab_pnl_usd {:.4}\n", m.utilidad_acumulada_usd));
        gauges.push_str(&format!("mayab_operaciones {}\n", m.operaciones));
        gauges.push_str(&format!(
            "mayab_operaciones_fallidas {}\n",
            m.operaciones_fallidas
        ));
        gauges.push_str(&format!(
            "mayab_oportunidades {}\n",
            estado.oportunidades.len()
        ));
        gauges.push_str(&format!("mayab_exchanges_activos {}\n", exchanges_activos));
        gauges.push_str(&format!("mayab_feeds_conectados {}\n", ws_conectados));
        gauges.push_str(&format!("mayab_circuit_breaker {}\n", circuit));
        gauges.push_str(&format!(
            "mayab_latencia_promedio_ms {:.3}\n",
            m.latencia_promedio_ms
        ));
        gauges.push_str(&format!("mayab_drawdown_usd {:.4}\n", m.max_drawdown_usd));
        gauges.push_str(&format!("mayab_sharpe {:.4}\n", m.sharpe_ratio));
        gauges.push_str(&format!("mayab_win_rate {:.4}\n", m.win_rate));
        gauges.push_str(&format!("mayab_rebalanceos {}\n", m.rebalanceos_totales));
        gauges.push_str(&format!(
            "mayab_auditorias {}\n",
            estado.auditoria_decisiones.len()
        ));
        if let Some(ga) = &estado.genetico {
            gauges.push_str(&format!("mayab_ga_generacion {}\n", ga.generacion));
            gauges.push_str(&format!("mayab_ga_poblacion {}\n", ga.poblacion));
            gauges.push_str(&format!("mayab_ga_diversidad {:.4}\n", ga.diversidad));
            gauges.push_str(&format!("mayab_ga_fitness {:.4}\n", ga.mejor_fitness));
        }
        if let Some(p) = &estado.persistencia {
            gauges.push_str(&format!(
                "mayab_persistencia_activa {}\n",
                if p.activa { 1 } else { 0 }
            ));
        }
        emit(
            "mayab_engine",
            "Métricas de motor proyectadas desde el estado público.",
            "gauge",
            &gauges,
        );

        out
    }

    /// Instante para medir latencia en middleware.
    pub fn ahora() -> Instant {
        Instant::now()
    }
}
