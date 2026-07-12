//! Modelos comparables de impacto para ejecución simulada.

use crate::types::NivelOrden;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LadoOrden {
    Compra,
    Venta,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ModeloImpacto {
    BookWalk,
    SquareRoot {
        eta: f64,
        volumen_diario_btc: f64,
    },
    AlmgrenLite {
        impacto_temporal: f64,
        impacto_permanente: f64,
        horizonte_ms: u64,
    },
}

impl Default for ModeloImpacto {
    fn default() -> Self {
        Self::BookWalk
    }
}

impl ModeloImpacto {
    pub fn nombre(&self) -> &'static str {
        match self {
            Self::BookWalk => "Book-walk",
            Self::SquareRoot { .. } => "Square-root",
            Self::AlmgrenLite { .. } => "Almgren-lite",
        }
    }
    pub fn estimar(&self, o: &OrdenImpacto<'_>) -> EstimacionImpacto {
        if !o.cantidad_btc.is_finite() || o.cantidad_btc <= 0.0 {
            return EstimacionImpacto::sin_datos("cantidad_btc debe ser positiva");
        }
        if !o.precio_referencia.is_finite() || o.precio_referencia <= 0.0 {
            return EstimacionImpacto::sin_datos("precio_referencia debe ser positivo");
        }
        match self {
            Self::BookWalk => book_walk(o),
            Self::SquareRoot {
                eta,
                volumen_diario_btc,
            } => square_root(o, *eta, *volumen_diario_btc),
            Self::AlmgrenLite {
                impacto_temporal,
                impacto_permanente,
                horizonte_ms,
            } => almgren_lite(o, *impacto_temporal, *impacto_permanente, *horizonte_ms),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct OrdenImpacto<'a> {
    pub lado: LadoOrden,
    pub cantidad_btc: f64,
    pub precio_referencia: f64,
    pub niveles: &'a [NivelOrden],
    pub volatilidad_bps: Option<f64>,
    pub horizonte_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EstimacionImpacto {
    #[serde(rename = "precioPromedioEsperado")]
    pub precio_promedio_esperado: f64,
    #[serde(rename = "impactoBps")]
    pub impacto_bps: f64,
    #[serde(rename = "slippageUsd")]
    pub slippage_usd: f64,
    #[serde(rename = "cantidadEjecutable")]
    pub cantidad_ejecutable: f64,
    pub confianza: f64,
    #[serde(rename = "motivoFaltanDatos", skip_serializing_if = "Option::is_none")]
    pub motivo_faltan_datos: Option<String>,
}

impl EstimacionImpacto {
    fn sin_datos(m: impl Into<String>) -> Self {
        Self {
            precio_promedio_esperado: 0.0,
            impacto_bps: 0.0,
            slippage_usd: 0.0,
            cantidad_ejecutable: 0.0,
            confianza: 0.0,
            motivo_faltan_datos: Some(m.into()),
        }
    }
}

fn resultado(o: &OrdenImpacto<'_>, cantidad: f64, bps: f64, confianza: f64) -> EstimacionImpacto {
    let signo = if o.lado == LadoOrden::Compra {
        1.0
    } else {
        -1.0
    };
    EstimacionImpacto {
        precio_promedio_esperado: o.precio_referencia * (1.0 + signo * bps / 10_000.0),
        impacto_bps: bps,
        slippage_usd: cantidad * o.precio_referencia * bps / 10_000.0,
        cantidad_ejecutable: cantidad,
        confianza: confianza.clamp(0.0, 1.0),
        motivo_faltan_datos: None,
    }
}

fn book_walk(o: &OrdenImpacto<'_>) -> EstimacionImpacto {
    let mut restante = o.cantidad_btc;
    let mut cantidad = 0.0;
    let mut nocional = 0.0;
    let mut usados = 0_u32;
    for n in o.niveles.iter().filter(|n| {
        n.precio.is_finite() && n.precio > 0.0 && n.cantidad.is_finite() && n.cantidad > 0.0
    }) {
        if restante <= 0.0 {
            break;
        }
        let tomar = restante.min(n.cantidad);
        cantidad += tomar;
        nocional += tomar * n.precio;
        restante -= tomar;
        usados += 1;
    }
    if cantidad <= 0.0 {
        return EstimacionImpacto::sin_datos("order book sin niveles validos");
    }
    let promedio = nocional / cantidad;
    let bps = match o.lado {
        LadoOrden::Compra => (promedio / o.precio_referencia - 1.0) * 10_000.0,
        LadoOrden::Venta => (1.0 - promedio / o.precio_referencia) * 10_000.0,
    }
    .max(0.0);
    let cobertura = (cantidad / o.cantidad_btc).clamp(0.0, 1.0);
    let mut r = resultado(
        o,
        cantidad,
        bps,
        cobertura * (0.75 + 0.05 * usados.min(5) as f64),
    );
    r.precio_promedio_esperado = promedio;
    if restante > 1e-12 {
        r.motivo_faltan_datos = Some(format!(
            "profundidad insuficiente: faltan {:.8} BTC",
            restante
        ));
    }
    r
}

fn square_root(o: &OrdenImpacto<'_>, eta: f64, volumen: f64) -> EstimacionImpacto {
    let Some(vol) = o.volatilidad_bps.filter(|v| v.is_finite() && *v > 0.0) else {
        return EstimacionImpacto::sin_datos("falta volatilidad_bps");
    };
    if !eta.is_finite() || eta < 0.0 || !volumen.is_finite() || volumen <= 0.0 {
        return EstimacionImpacto::sin_datos("eta o volumen_diario_btc invalidos");
    }
    resultado(
        o,
        o.cantidad_btc,
        eta * vol * (o.cantidad_btc / volumen).sqrt(),
        0.72,
    )
}

fn almgren_lite(
    o: &OrdenImpacto<'_>,
    temporal: f64,
    permanente: f64,
    horizonte_modelo: u64,
) -> EstimacionImpacto {
    let Some(vol) = o.volatilidad_bps.filter(|v| v.is_finite() && *v > 0.0) else {
        return EstimacionImpacto::sin_datos("falta volatilidad_bps");
    };
    let Some(horizonte) = o.horizonte_ms.filter(|h| *h > 0) else {
        return EstimacionImpacto::sin_datos("falta horizonte_ms de la orden");
    };
    if horizonte_modelo == 0
        || !temporal.is_finite()
        || temporal < 0.0
        || !permanente.is_finite()
        || permanente < 0.0
    {
        return EstimacionImpacto::sin_datos("parametros Almgren-lite invalidos");
    }
    let bps = vol
        * o.cantidad_btc.sqrt()
        * (temporal * (horizonte_modelo as f64 / horizonte as f64).sqrt() + permanente);
    resultado(o, o.cantidad_btc, bps, 0.68)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn book_walk_reporta_fill_parcial_y_vwap() {
        let n = vec![
            NivelOrden {
                precio: 100.0,
                cantidad: 1.0,
            },
            NivelOrden {
                precio: 102.0,
                cantidad: 1.0,
            },
        ];
        let r = ModeloImpacto::BookWalk.estimar(&OrdenImpacto {
            lado: LadoOrden::Compra,
            cantidad_btc: 3.0,
            precio_referencia: 100.0,
            niveles: &n,
            volatilidad_bps: None,
            horizonte_ms: None,
        });
        assert_eq!(r.cantidad_ejecutable, 2.0);
        assert_eq!(r.precio_promedio_esperado, 101.0);
        assert!((r.impacto_bps - 100.0).abs() < 1e-9);
        assert!(r.motivo_faltan_datos.is_some());
    }
    #[test]
    fn explica_datos_faltantes() {
        let o = OrdenImpacto {
            lado: LadoOrden::Venta,
            cantidad_btc: 0.1,
            precio_referencia: 100_000.0,
            niveles: &[],
            volatilidad_bps: None,
            horizonte_ms: None,
        };
        let r = ModeloImpacto::SquareRoot {
            eta: 0.5,
            volumen_diario_btc: 10_000.0,
        }
        .estimar(&o);
        assert_eq!(r.confianza, 0.0);
        assert!(r.motivo_faltan_datos.unwrap().contains("volatilidad"));
    }
    #[test]
    fn venta_mueve_precio_abajo() {
        let o = OrdenImpacto {
            lado: LadoOrden::Venta,
            cantidad_btc: 0.25,
            precio_referencia: 100_000.0,
            niveles: &[],
            volatilidad_bps: Some(50.0),
            horizonte_ms: Some(500),
        };
        let r = ModeloImpacto::SquareRoot {
            eta: 1.0,
            volumen_diario_btc: 1_000.0,
        }
        .estimar(&o);
        assert!(r.precio_promedio_esperado < o.precio_referencia);
        assert!(r.slippage_usd > 0.0);
    }

    #[test]
    fn book_walk_es_el_default() {
        assert_eq!(ModeloImpacto::default(), ModeloImpacto::BookWalk);
    }
}
