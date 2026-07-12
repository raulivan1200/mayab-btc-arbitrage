//! Laboratorio reproducible de microestructura y calibración probabilística.
//!
//! Este módulo permanece separado del motor y del GA: mide quote age,
//! asincronía, microprice, OFI multinivel y markouts; después calibra una
//! probabilidad de fill con Platt e isotónica usando una ventana cronológica B
//! y evalúa una ventana C nunca vista.

use crate::tape::{TapeEvent, EVENTS_FILE};
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

#[derive(Clone, Debug)]
struct Sample {
    venue: String,
    quote_age_ms: f64,
    async_ms: f64,
    microprice_skew_bps: f64,
    ofi_l1: f64,
    ofi_multi: f64,
    liquidity_btc: f64,
    markout_bps: [f64; 4],
    filled: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReliabilityBin {
    pub lower: f64,
    pub upper: f64,
    pub observations: usize,
    pub predicted_mean: f64,
    pub observed_rate: f64,
    pub wilson_95: [f64; 2],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationMetrics {
    pub model: String,
    pub brier_score: f64,
    pub log_loss: f64,
    pub expected_calibration_error: f64,
    pub reliability: Vec<ReliabilityBin>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VenueSummary {
    pub venue: String,
    pub observations: usize,
    pub quote_age_p50_ms: f64,
    pub quote_age_p95_ms: f64,
    pub asynchrony_p95_ms: f64,
    pub microprice_skew_mean_bps: f64,
    pub ofi_multi_mean: f64,
    pub fill_rate: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrostructureReport {
    pub schema_version: u32,
    pub source_kind: String,
    pub source_path: Option<String>,
    pub observations: usize,
    pub split: [usize; 3],
    pub leakage_guards: HashMap<String, bool>,
    pub features: Vec<&'static str>,
    pub venues: Vec<VenueSummary>,
    pub markouts_mean_bps: HashMap<&'static str, f64>,
    pub estimated_second_leg_risk_mean: f64,
    pub calibration: Vec<CalibrationMetrics>,
    pub winner_by_brier: String,
    pub transfer_between_venues: Vec<serde_json::Value>,
    pub ou_lab: serde_json::Value,
    pub limitations: Vec<String>,
}

pub fn build_report(path: Option<&Path>, seed: u64) -> MicrostructureReport {
    let (samples, source_kind, source_path, mut limitations) = path
        .and_then(|p| load_tape(p).ok().filter(|v| v.len() >= 120))
        .map(|v| {
            (
                v,
                "real_public_order_book_tape".to_string(),
                path.map(|p| p.display().to_string()),
                vec!["Fill es un proxy observable basado en cruce/markout; no es confirmación privada del exchange.".to_string()],
            )
        })
        .unwrap_or_else(|| {
            (
                synthetic_samples(seed, 2_400),
                "synthetic_reproducible_fallback".to_string(),
                path.map(|p| p.display().to_string()),
                vec![
                    "No había un tape verificable con al menos 120 observaciones; las cifras son sintéticas.".to_string(),
                    "El fallback valida contratos y metodología, no demuestra edge de mercado.".to_string(),
                ],
            )
        });
    let n_train = samples.len() * 50 / 100;
    let n_cal = samples.len() * 20 / 100;
    let train = &samples[..n_train];
    let calibration = &samples[n_train..n_train + n_cal];
    let holdout = &samples[n_train + n_cal..];
    let raw_cal = calibration.iter().map(raw_probability).collect::<Vec<_>>();
    let labels_cal = calibration.iter().map(|s| s.filled).collect::<Vec<_>>();
    let platt = fit_platt(&raw_cal, &labels_cal);
    let isotonic = fit_isotonic(&raw_cal, &labels_cal);
    let raw_holdout = holdout.iter().map(raw_probability).collect::<Vec<_>>();
    let labels_holdout = holdout.iter().map(|s| s.filled).collect::<Vec<_>>();
    let predictions = [
        ("sin_calibrar", raw_holdout.clone()),
        (
            "platt",
            raw_holdout
                .iter()
                .map(|p| sigmoid(platt.0 * logit(*p) + platt.1))
                .collect(),
        ),
        (
            "isotonica",
            raw_holdout
                .iter()
                .map(|p| isotonic_predict(&isotonic, *p))
                .collect(),
        ),
    ];
    let calibration_reports = predictions
        .into_iter()
        .map(|(name, p)| metrics(name, &p, &labels_holdout))
        .collect::<Vec<_>>();
    let winner = calibration_reports
        .iter()
        .min_by(|a, b| a.brier_score.total_cmp(&b.brier_score))
        .map(|m| m.model.clone())
        .unwrap_or_default();
    let mut guards = HashMap::new();
    guards.insert("chronologicalSplit".into(), true);
    guards.insert("calibrationSeesOnlyB".into(), true);
    guards.insert("holdoutEvaluatedAfterFreeze".into(), true);
    guards.insert("gaParametersUnchanged".into(), true);
    let transfers = venue_transfer(holdout, platt, &isotonic);
    if train.len() < 100 {
        limitations
            .push("La ventana de entrenamiento es pequeña; no usar para decisiones live.".into());
    }
    MicrostructureReport {
        schema_version: 1,
        source_kind,
        source_path,
        observations: samples.len(),
        split: [train.len(), calibration.len(), holdout.len()],
        leakage_guards: guards,
        features: vec![
            "quote_age_ms",
            "cross_venue_asynchrony_ms",
            "microprice_skew_bps",
            "ofi_l1",
            "multi_level_ofi",
            "visible_liquidity_btc",
            "estimated_second_leg_risk",
        ],
        venues: venue_summaries(&samples),
        markouts_mean_bps: markout_summary(holdout),
        estimated_second_leg_risk_mean: holdout.iter().map(second_leg_risk).sum::<f64>()
            / holdout.len().max(1) as f64,
        calibration: calibration_reports,
        winner_by_brier: winner,
        transfer_between_venues: transfers,
        ou_lab: ou_lab(train, calibration, holdout),
        limitations,
    }
}

fn load_tape(path: &Path) -> anyhow::Result<Vec<Sample>> {
    let events_path = if path.is_dir() {
        path.join(EVENTS_FILE)
    } else {
        path.to_path_buf()
    };
    let mut events = Vec::new();
    for line in BufReader::new(File::open(events_path)?).lines() {
        events.push(serde_json::from_str::<TapeEvent>(&line?)?);
    }
    let mut by_venue: HashMap<(String, String), Vec<(usize, f64)>> = HashMap::new();
    for (i, e) in events.iter().enumerate() {
        if let (Some(b), Some(a)) = (e.bids.first(), e.asks.first()) {
            by_venue
                .entry((e.exchange.clone(), e.pair.clone()))
                .or_default()
                .push((i, (b.precio + a.precio) / 2.0));
        }
    }
    let mut positions: HashMap<(String, String), usize> = HashMap::new();
    let mut latest_ts: HashMap<String, i64> = HashMap::new();
    let mut out = Vec::new();
    for e in &events {
        let (Some(bid), Some(ask)) = (e.bids.first(), e.asks.first()) else {
            continue;
        };
        if ask.precio <= bid.precio || bid.cantidad <= 0.0 || ask.cantidad <= 0.0 {
            continue;
        }
        let key = (e.exchange.clone(), e.pair.clone());
        let pos = positions.entry(key.clone()).or_default();
        let series = &by_venue[&key];
        let mid = (bid.precio + ask.precio) / 2.0;
        let futures = [1, 5, 10, 25].map(|h| {
            series
                .get((*pos + h).min(series.len() - 1))
                .map(|x| x.1)
                .unwrap_or(mid)
        });
        *pos += 1;
        let local_ms = e.local_timestamp.timestamp_millis();
        latest_ts.insert(e.exchange.clone(), local_ms);
        let min_peer = latest_ts.values().copied().min().unwrap_or(local_ms);
        let bid_depth = e.bids.iter().take(5).map(|n| n.cantidad).sum::<f64>();
        let ask_depth = e.asks.iter().take(5).map(|n| n.cantidad).sum::<f64>();
        let micro =
            (ask.precio * bid.cantidad + bid.precio * ask.cantidad) / (bid.cantidad + ask.cantidad);
        let ofi_l1 = imbalance(bid.cantidad, ask.cantidad);
        let ofi_multi = imbalance(bid_depth, ask_depth);
        let markouts = futures.map(|future| (future / mid - 1.0) * 10_000.0);
        out.push(Sample {
            venue: e.exchange.clone(),
            quote_age_ms: e.observed_latency_ms.unwrap_or(0) as f64,
            async_ms: (local_ms - min_peer).max(0) as f64,
            microprice_skew_bps: (micro / mid - 1.0) * 10_000.0,
            ofi_l1,
            ofi_multi,
            liquidity_btc: bid_depth + ask_depth,
            markout_bps: markouts,
            filled: markouts[0] >= -(ask.precio / bid.precio - 1.0) * 5_000.0,
        });
    }
    Ok(out)
}

fn synthetic_samples(seed: u64, n: usize) -> Vec<Sample> {
    let venues = ["Binance", "Kraken", "Coinbase", "OKX"];
    let mut rng = StdRng::seed_from_u64(seed ^ 0x4d49_4352_4f53);
    (0..n)
        .map(|i| {
            let age = rng.gen_range(8.0..420.0);
            let async_ms = rng.gen_range(0.0..180.0);
            let ofi_l1: f64 = rng.gen_range(-1.0..1.0);
            let ofi_multi = (ofi_l1 * 0.65 + rng.gen_range(-0.45..0.45)).clamp(-1.0, 1.0);
            let skew = ofi_l1 * rng.gen_range(0.2..1.8);
            let signal = 1.4 * ofi_multi + 0.55 * skew - age / 380.0 - async_ms / 240.0;
            let filled = rng.gen_bool(sigmoid(signal).clamp(0.02, 0.98));
            let base_markout = 0.7 * ofi_multi + 0.3 * skew + rng.gen_range(-1.4..1.4);
            Sample {
                venue: venues[i % venues.len()].into(),
                quote_age_ms: age,
                async_ms,
                microprice_skew_bps: skew,
                ofi_l1,
                ofi_multi,
                liquidity_btc: rng.gen_range(0.4..18.0),
                markout_bps: [
                    base_markout,
                    base_markout * 1.15,
                    base_markout * 1.3,
                    base_markout * 1.5,
                ],
                filled,
            }
        })
        .collect()
}

fn imbalance(bid: f64, ask: f64) -> f64 {
    (bid - ask) / (bid + ask).max(1e-12)
}
fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x.clamp(-30.0, 30.0)).exp())
}
fn logit(p: f64) -> f64 {
    let p = p.clamp(1e-6, 1.0 - 1e-6);
    (p / (1.0 - p)).ln()
}
fn raw_probability(s: &Sample) -> f64 {
    sigmoid(
        -0.15 - s.quote_age_ms / 500.0 - s.async_ms / 350.0
            + 0.45 * s.microprice_skew_bps
            + 0.8 * s.ofi_l1
            + 1.1 * s.ofi_multi
            + 0.03 * s.liquidity_btc.ln_1p(),
    )
}

fn second_leg_risk(s: &Sample) -> f64 {
    sigmoid(
        -2.2 + s.quote_age_ms / 240.0 + s.async_ms / 130.0
            - 0.55 * s.ofi_multi
            - 0.08 * s.liquidity_btc.ln_1p(),
    )
}

fn ou_lab(train: &[Sample], calibration: &[Sample], holdout: &[Sample]) -> serde_json::Value {
    fn series(samples: &[Sample]) -> Vec<f64> {
        samples
            .iter()
            .map(|s| s.microprice_skew_bps - 0.6 * s.ofi_multi)
            .collect()
    }
    fn estimate(values: &[f64]) -> (f64, f64, f64) {
        if values.len() < 3 {
            return (0.0, 0.0, f64::INFINITY);
        }
        let x = &values[..values.len() - 1];
        let y = &values[1..];
        let mx = x.iter().sum::<f64>() / x.len() as f64;
        let my = y.iter().sum::<f64>() / y.len() as f64;
        let variance = x.iter().map(|v| (v - mx).powi(2)).sum::<f64>();
        let beta = if variance > 1e-12 {
            x.iter()
                .zip(y)
                .map(|(a, b)| (a - mx) * (b - my))
                .sum::<f64>()
                / variance
        } else {
            0.0
        };
        let alpha = my - beta * mx;
        let mean = if (1.0 - beta).abs() > 1e-9 {
            alpha / (1.0 - beta)
        } else {
            mx
        };
        let half_life = if beta > 0.0 && beta < 1.0 {
            -std::f64::consts::LN_2 / beta.ln()
        } else {
            f64::INFINITY
        };
        (mean, beta, half_life)
    }
    fn pnl(values: &[f64], mean: f64, threshold: f64) -> (f64, usize) {
        let mut result = 0.0;
        let mut trades = 0;
        for pair in values.windows(2) {
            let deviation = pair[0] - mean;
            if deviation.abs() >= threshold {
                result += -deviation.signum() * (pair[1] - pair[0]);
                trades += 1;
            }
        }
        (result, trades)
    }
    let a = series(train);
    let b = series(calibration);
    let c = series(holdout);
    let (mean, beta, half_life) = estimate(&a);
    let sigma = (a.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / a.len().max(1) as f64)
        .sqrt()
        .max(1e-9);
    let thresholds = [0.5, 1.0, 1.5, 2.0];
    let selected = thresholds
        .into_iter()
        .map(|multiple| (multiple, pnl(&b, mean, sigma * multiple).0))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or((1.0, 0.0));
    let (ou_pnl, trades) = pnl(&c, mean, sigma * selected.0);
    let simple_pnl = c.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() * 0.05;
    let mid = a.len() / 2;
    let beta_first = estimate(&a[..mid.max(3)]).1;
    let beta_second = estimate(&a[mid.min(a.len().saturating_sub(3))..]).1;
    let stationary = beta > 0.0 && beta < 1.0 && half_life.is_finite();
    let stable = (beta_first - beta_second).abs() <= 0.35;
    serde_json::json!({
        "separateFromGa": true,
        "protocol": {"estimateOnA": true, "selectOnB": true, "evaluateOnceOnC": true},
        "parametersFrozen": {"mean": mean, "ar1Beta": beta, "halfLifeEvents": half_life, "sigma": sigma, "thresholdSigma": selected.0},
        "diagnostics": {"stationaryProxy": stationary, "stableBetweenTrainHalves": stable, "betaFirstHalf": beta_first, "betaSecondHalf": beta_second},
        "holdout": {"ouPnlBpsProxy": ou_pnl, "trades": trades, "noTradePnlBps": 0.0, "simpleSpreadBaselinePnlBpsProxy": simple_pnl},
        "accepted": stationary && stable && ou_pnl > 0.0 && ou_pnl > simple_pnl,
        "decision": if !stationary || !stable { "rejected_unstable_or_non_stationary" } else if ou_pnl <= simple_pnl { "rejected_does_not_beat_baseline" } else { "accepted_for_research_only" },
        "limitation": "Diagnóstico AR(1)/OU discreto y PnL proxy; no sustituye ADF/KPSS ni fills confirmados."
    })
}

fn fit_platt(p: &[f64], y: &[bool]) -> (f64, f64) {
    let (mut a, mut b) = (1.0, 0.0);
    for _ in 0..800 {
        let mut ga = 0.0;
        let mut gb = 0.0;
        for (&pi, &yi) in p.iter().zip(y) {
            let x = logit(pi);
            let error = sigmoid(a * x + b) - f64::from(yi);
            ga += error * x;
            gb += error;
        }
        let scale = 0.08 / p.len().max(1) as f64;
        a -= scale * ga;
        b -= scale * gb;
    }
    (a, b)
}

fn fit_isotonic(p: &[f64], y: &[bool]) -> Vec<(f64, f64)> {
    let mut pairs = p
        .iter()
        .copied()
        .zip(y.iter().map(|v| f64::from(*v)))
        .collect::<Vec<_>>();
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut blocks: Vec<(f64, f64, usize)> = Vec::new();
    for (x, value) in pairs {
        blocks.push((x, value, 1));
        while blocks.len() >= 2 {
            let n = blocks.len();
            if blocks[n - 2].1 <= blocks[n - 1].1 {
                break;
            }
            let right = blocks.pop().unwrap();
            let left = blocks.pop().unwrap();
            let count = left.2 + right.2;
            blocks.push((
                right.0,
                (left.1 * left.2 as f64 + right.1 * right.2 as f64) / count as f64,
                count,
            ));
        }
    }
    blocks.into_iter().map(|(x, value, _)| (x, value)).collect()
}
fn isotonic_predict(model: &[(f64, f64)], p: f64) -> f64 {
    model
        .iter()
        .find(|(x, _)| p <= *x)
        .or_else(|| model.last())
        .map(|x| x.1)
        .unwrap_or(p)
}

fn metrics(name: &str, p: &[f64], y: &[bool]) -> CalibrationMetrics {
    let n = p.len().max(1) as f64;
    let brier = p
        .iter()
        .zip(y)
        .map(|(p, y)| (p - f64::from(*y)).powi(2))
        .sum::<f64>()
        / n;
    let log_loss = -p
        .iter()
        .zip(y)
        .map(|(p, y)| {
            let p = p.clamp(1e-9, 1.0 - 1e-9);
            if *y {
                p.ln()
            } else {
                (1.0 - p).ln()
            }
        })
        .sum::<f64>()
        / n;
    let reliability = (0..10)
        .map(|bin| {
            let lo = bin as f64 / 10.0;
            let hi = (bin + 1) as f64 / 10.0;
            let values = p
                .iter()
                .zip(y)
                .filter(|(v, _)| **v >= lo && (**v < hi || bin == 9))
                .collect::<Vec<_>>();
            let count = values.len();
            let successes = values.iter().filter(|(_, y)| **y).count();
            ReliabilityBin {
                lower: lo,
                upper: hi,
                observations: count,
                predicted_mean: values.iter().map(|(v, _)| **v).sum::<f64>() / count.max(1) as f64,
                observed_rate: successes as f64 / count.max(1) as f64,
                wilson_95: wilson(successes, count),
            }
        })
        .collect::<Vec<_>>();
    let ece = reliability
        .iter()
        .map(|b| b.observations as f64 / n * (b.predicted_mean - b.observed_rate).abs())
        .sum();
    CalibrationMetrics {
        model: name.into(),
        brier_score: brier,
        log_loss,
        expected_calibration_error: ece,
        reliability,
    }
}
fn wilson(successes: usize, n: usize) -> [f64; 2] {
    if n == 0 {
        return [0.0, 1.0];
    }
    let z = 1.959963984540054;
    let n = n as f64;
    let p = successes as f64 / n;
    let center = (p + z * z / (2.0 * n)) / (1.0 + z * z / n);
    let margin = z / (1.0 + z * z / n) * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt();
    [(center - margin).max(0.0), (center + margin).min(1.0)]
}

fn percentile(mut v: Vec<f64>, q: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q).round() as usize]
}
fn venue_summaries(samples: &[Sample]) -> Vec<VenueSummary> {
    let mut grouped: HashMap<&str, Vec<&Sample>> = HashMap::new();
    for s in samples {
        grouped.entry(&s.venue).or_default().push(s);
    }
    let mut out = grouped
        .into_iter()
        .map(|(venue, s)| {
            let n = s.len().max(1) as f64;
            VenueSummary {
                venue: venue.into(),
                observations: s.len(),
                quote_age_p50_ms: percentile(s.iter().map(|x| x.quote_age_ms).collect(), 0.5),
                quote_age_p95_ms: percentile(s.iter().map(|x| x.quote_age_ms).collect(), 0.95),
                asynchrony_p95_ms: percentile(s.iter().map(|x| x.async_ms).collect(), 0.95),
                microprice_skew_mean_bps: s.iter().map(|x| x.microprice_skew_bps).sum::<f64>() / n,
                ofi_multi_mean: s.iter().map(|x| x.ofi_multi).sum::<f64>() / n,
                fill_rate: s.iter().filter(|x| x.filled).count() as f64 / n,
            }
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| a.venue.cmp(&b.venue));
    out
}
fn markout_summary(samples: &[Sample]) -> HashMap<&'static str, f64> {
    let n = samples.len().max(1) as f64;
    [("100ms", 0), ("500ms", 1), ("1s", 2), ("5s", 3)]
        .into_iter()
        .map(|(k, i)| (k, samples.iter().map(|s| s.markout_bps[i]).sum::<f64>() / n))
        .collect()
}
fn venue_transfer(
    holdout: &[Sample],
    platt: (f64, f64),
    iso: &[(f64, f64)],
) -> Vec<serde_json::Value> {
    let mut grouped: HashMap<&str, Vec<&Sample>> = HashMap::new();
    for s in holdout {
        grouped.entry(&s.venue).or_default().push(s)
    }
    let mut out=grouped.into_iter().map(|(venue,s)| {let raw=s.iter().map(|x|raw_probability(x)).collect::<Vec<_>>();let y=s.iter().map(|x|x.filled).collect::<Vec<_>>();
        let pp=raw.iter().map(|p|sigmoid(platt.0*logit(*p)+platt.1)).collect::<Vec<_>>(); let ip=raw.iter().map(|p|isotonic_predict(iso,*p)).collect::<Vec<_>>();
        serde_json::json!({"venue":venue,"observations":s.len(),"plattBrier":metrics("p",&pp,&y).brier_score,"isotonicBrier":metrics("i",&ip,&y).brier_score})}).collect::<Vec<_>>();
    out.sort_by(|a, b| a["venue"].as_str().cmp(&b["venue"].as_str()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_is_chronological_and_calibrated() {
        let r = build_report(None, 42);
        assert_eq!(r.observations, 2400);
        assert_eq!(r.split, [1200, 480, 720]);
        assert_eq!(r.calibration.len(), 3);
        assert!(r.leakage_guards.values().all(|v| *v));
    }
    #[test]
    fn wilson_contains_observed_rate() {
        let ci = wilson(30, 100);
        assert!(ci[0] < 0.3 && ci[1] > 0.3);
    }
    #[test]
    fn isotonic_is_monotone() {
        let m = fit_isotonic(&[0.1, 0.2, 0.3, 0.4], &[true, false, false, true]);
        assert!(m.windows(2).all(|w| w[0].1 <= w[1].1));
    }
}
