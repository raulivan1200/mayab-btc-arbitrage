//! Métricas Prometheus acotadas y sin dependencias externas.
//!
//! Las etiquetas proceden de catálogos cerrados (ruta HTTP o etapa interna),
//! evitando símbolos, IDs de operación o mensajes de error de alta cardinalidad.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use crate::types::EstadoPublico;

const BUCKETS_MS: [f64; 10] = [0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 500.0];

#[derive(Clone, Default)]
pub struct Metricas {
    inner: Arc<MetricasInner>,
}

#[derive(Default)]
struct MetricasInner {
    http_requests_total: Mutex<HashMap<(String, String, u16), u64>>,
    http_duration: Mutex<HashMap<String, Histograma>>,
    stage_duration: Mutex<HashMap<String, Histograma>>,
    stage_events: Mutex<HashMap<String, u64>>,
}

#[derive(Clone, Debug, Default)]
struct Histograma {
    buckets: [u64; BUCKETS_MS.len()],
    count: u64,
    sum_ms: f64,
}

impl Histograma {
    fn observe(&mut self, ms: f64) {
        self.count += 1;
        self.sum_ms += ms;
        for (index, limit) in BUCKETS_MS.iter().enumerate() {
            if ms <= *limit {
                self.buckets[index] += 1;
            }
        }
    }
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
        *self
            .inner
            .http_requests_total
            .lock()
            .unwrap()
            .entry((metodo.to_string(), ruta.to_string(), status))
            .or_insert(0) += 1;
        self.inner
            .http_duration
            .lock()
            .unwrap()
            .entry(ruta.to_string())
            .or_default()
            .observe(duracion.as_secs_f64() * 1000.0);
    }

    /// Registra una etapa del pipeline. `etapa` debe pertenecer a un catálogo
    /// estático definido por el llamador; nunca debe contener datos de mercado.
    pub fn registrar_etapa(&self, etapa: &'static str, duracion: std::time::Duration) {
        self.inner
            .stage_duration
            .lock()
            .unwrap()
            .entry(etapa.to_string())
            .or_default()
            .observe(duracion.as_secs_f64() * 1000.0);
        *self
            .inner
            .stage_events
            .lock()
            .unwrap()
            .entry(etapa.to_string())
            .or_insert(0) += 1;
    }

    fn render_histogram(
        out: &mut String,
        name: &str,
        label: &str,
        values: &HashMap<String, Histograma>,
    ) {
        for (value, histogram) in values {
            for (index, limit) in BUCKETS_MS.iter().enumerate() {
                out.push_str(&format!(
                    "{name}_bucket{{{label}=\"{value}\",le=\"{limit}\"}} {}\n",
                    histogram.buckets[index]
                ));
            }
            out.push_str(&format!(
                "{name}_bucket{{{label}=\"{value}\",le=\"+Inf\"}} {}\n",
                histogram.count
            ));
            out.push_str(&format!(
                "{name}_sum{{{label}=\"{value}\"}} {:.6}\n",
                histogram.sum_ms
            ));
            out.push_str(&format!(
                "{name}_count{{{label}=\"{value}\"}} {}\n",
                histogram.count
            ));
        }
    }

    pub fn render(&self, estado: &EstadoPublico) -> String {
        let mut out = String::new();
        out.push_str("# HELP mayab_http_requests_total Peticiones HTTP por ruta, metodo y status.\n# TYPE mayab_http_requests_total counter\n");
        for ((metodo, ruta, status), n) in self.inner.http_requests_total.lock().unwrap().iter() {
            out.push_str(&format!("mayab_http_requests_total{{metodo=\"{metodo}\",ruta=\"{ruta}\",status=\"{status}\"}} {n}\n"));
        }
        out.push_str("# HELP mayab_http_request_duration_ms Duracion HTTP en milisegundos.\n# TYPE mayab_http_request_duration_ms histogram\n");
        Self::render_histogram(
            &mut out,
            "mayab_http_request_duration_ms",
            "ruta",
            &self.inner.http_duration.lock().unwrap(),
        );
        out.push_str("# HELP mayab_stage_duration_ms Duracion por etapa interna del pipeline.\n# TYPE mayab_stage_duration_ms histogram\n");
        Self::render_histogram(
            &mut out,
            "mayab_stage_duration_ms",
            "etapa",
            &self.inner.stage_duration.lock().unwrap(),
        );
        out.push_str("# HELP mayab_stage_events_total Eventos procesados por etapa interna.\n# TYPE mayab_stage_events_total counter\n");
        for (stage, count) in self.inner.stage_events.lock().unwrap().iter() {
            out.push_str(&format!(
                "mayab_stage_events_total{{etapa=\"{stage}\"}} {count}\n"
            ));
        }

        let m = &estado.metricas;
        let active = estado.exchanges_activos.values().filter(|v| **v).count();
        let connected = estado.cotizaciones.iter().filter(|c| c.conectado).count();
        out.push_str("# HELP mayab_engine Estado operativo proyectado por el motor.\n# TYPE mayab_engine gauge\n");
        out.push_str(&format!("mayab_pnl_usd {:.4}\nmayab_operaciones {}\nmayab_operaciones_fallidas {}\nmayab_oportunidades {}\nmayab_exchanges_activos {}\nmayab_feeds_conectados {}\nmayab_circuit_breaker {}\nmayab_latencia_promedio_ms {:.3}\nmayab_drawdown_usd {:.4}\nmayab_sharpe {:.4}\nmayab_win_rate {:.4}\nmayab_rebalanceos {}\nmayab_auditorias {}\n",
            m.utilidad_acumulada_usd, m.operaciones, m.operaciones_fallidas, estado.oportunidades.len(), active, connected,
            u8::from(m.circuit_breaker_activo), m.latencia_promedio_ms, m.max_drawdown_usd, m.sharpe_ratio, m.win_rate,
            m.rebalanceos_totales, estado.auditoria_decisiones.len()));
        if let Some(ga) = &estado.genetico {
            out.push_str(&format!("mayab_ga_generacion {}\nmayab_ga_poblacion {}\nmayab_ga_diversidad {:.4}\nmayab_ga_fitness {:.4}\n", ga.generacion, ga.poblacion, ga.diversidad, ga.mejor_fitness));
        }
        if let Some(p) = &estado.persistencia {
            out.push_str(&format!(
                "mayab_persistencia_activa {}\n",
                u8::from(p.activa)
            ));
        }
        out
    }

    pub fn ahora() -> Instant {
        Instant::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn histogram_is_cumulative_and_has_inf_bucket() {
        let mut histogram = Histograma::default();
        histogram.observe(1.0);
        histogram.observe(600.0);
        assert_eq!(histogram.buckets[2], 1);
        assert_eq!(histogram.buckets[9], 1);
        assert_eq!(histogram.count, 2);
    }
}
