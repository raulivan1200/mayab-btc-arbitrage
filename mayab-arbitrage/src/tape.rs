//! Tape reproducible de libros públicos y verificación offline.

use crate::{mercado, types::NivelOrden};
use anyhow::{bail, Context};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

type BookSides = (BTreeMap<i64, f64>, BTreeMap<i64, f64>);

pub const EVENTS_FILE: &str = "events.jsonl";
pub const MANIFEST_FILE: &str = "manifest.json";
pub const CONFIG_FILE: &str = "capture-config.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum TapeSource {
    WebSocket { url: String },
    Rest { url: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Snapshot,
    Delta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityState {
    pub status: String,
    pub gap: bool,
    pub resync: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TapeEvent {
    pub schema_version: u32,
    pub exchange_timestamp: Option<DateTime<Utc>>,
    pub local_timestamp: DateTime<Utc>,
    pub exchange: String,
    pub pair: String,
    pub source: TapeSource,
    pub kind: EventKind,
    pub sequence_id: Option<u64>,
    pub previous_sequence: Option<u64>,
    pub bids: Vec<NivelOrden>,
    pub asks: Vec<NivelOrden>,
    pub integrity: IntegrityState,
    pub observed_latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureConfig {
    pub schema_version: u32,
    pub pair: String,
    pub exchanges: Vec<String>,
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TapeManifest {
    pub schema_version: u32,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub exchanges: Vec<String>,
    pub pairs: Vec<String>,
    pub events: u64,
    pub snapshots: u64,
    pub sequence_gaps: u64,
    pub rest_fallback_events: u64,
    pub sha256: String,
    pub git_commit: String,
    pub config_sha256: String,
}

pub async fn capture(
    output: &Path,
    duration: Duration,
    config: CaptureConfig,
) -> anyhow::Result<TapeManifest> {
    if output.exists() {
        bail!("la salida ya existe: {}", output.display());
    }
    fs::create_dir_all(output)?;
    let config_bytes = serde_json::to_vec_pretty(&config)?;
    fs::write(output.join(CONFIG_FILE), &config_bytes)?;
    let config_sha256 = hex_sha(&config_bytes);
    let started_at = Utc::now();
    let (tx, mut rx) = tokio::sync::mpsc::channel(8192);
    mercado::capture_public_books(
        config.pair.clone(),
        config.exchanges.clone(),
        config.depth,
        tx,
    )
    .await?;
    let mut writer = BufWriter::new(File::create(output.join(EVENTS_FILE))?);
    let deadline = tokio::time::Instant::now() + duration;
    let mut events = 0;
    let mut snapshots = 0;
    let mut gaps = 0;
    let mut rest = 0;
    loop {
        let event = tokio::select! {
            _ = tokio::time::sleep_until(deadline) => break,
            event = rx.recv() => event.context("todos los capturadores terminaron")?,
            _ = tokio::signal::ctrl_c() => break,
        };
        serde_json::to_writer(&mut writer, &event)?;
        writer.write_all(b"\n")?;
        events += 1;
        snapshots += u64::from(event.kind == EventKind::Snapshot);
        gaps += u64::from(event.integrity.gap);
        rest += u64::from(matches!(event.source, TapeSource::Rest { .. }));
    }
    writer.flush()?;
    if events == 0 {
        bail!("captura vacía; verifica conectividad y exchanges");
    }
    let sha256 = file_sha(&output.join(EVENTS_FILE))?;
    let manifest = TapeManifest {
        schema_version: 1,
        started_at,
        ended_at: Utc::now(),
        exchanges: config.exchanges,
        pairs: vec![config.pair],
        events,
        snapshots,
        sequence_gaps: gaps,
        rest_fallback_events: rest,
        sha256,
        git_commit: git_commit(),
        config_sha256,
    };
    fs::write(
        output.join(MANIFEST_FILE),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(manifest)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    pub path: PathBuf,
    pub events: u64,
    pub books_reconstructed: usize,
    pub sha256: String,
}

pub fn verify(path: &Path) -> anyhow::Result<Verification> {
    let manifest: TapeManifest = serde_json::from_slice(
        &fs::read(path.join(MANIFEST_FILE)).context("falta manifest.json")?,
    )?;
    if manifest.schema_version != 1 {
        bail!("schemaVersion de manifiesto no soportada");
    }
    let config_bytes = fs::read(path.join(CONFIG_FILE)).context("falta capture-config.json")?;
    if hex_sha(&config_bytes) != manifest.config_sha256 {
        bail!("configSha256 no coincide");
    }
    let actual_sha = file_sha(&path.join(EVENTS_FILE))?;
    if actual_sha != manifest.sha256 {
        bail!("sha256 de events.jsonl no coincide");
    }
    let mut books: HashMap<(String, String), BookSides> = HashMap::new();
    let mut last_local = None;
    let mut last_seq: HashMap<(String, String), u64> = HashMap::new();
    let mut count = 0;
    let mut snapshots = 0;
    let mut gaps = 0;
    let mut rest = 0;
    for (line_no, line) in BufReader::new(File::open(path.join(EVENTS_FILE))?)
        .lines()
        .enumerate()
    {
        let event: TapeEvent = serde_json::from_str(&line?)
            .with_context(|| format!("evento inválido en línea {}", line_no + 1))?;
        if event.schema_version != 1 {
            bail!("schemaVersion inválida en línea {}", line_no + 1);
        }
        if last_local.is_some_and(|v| event.local_timestamp < v) {
            bail!("orden temporal inválido en línea {}", line_no + 1);
        }
        last_local = Some(event.local_timestamp);
        let key = (event.exchange.clone(), event.pair.clone());
        if let Some(seq) = event.sequence_id {
            if !event.integrity.resync && last_seq.get(&key).is_some_and(|old| seq < *old) {
                bail!("secuencia no monótona en línea {}", line_no + 1);
            }
            last_seq.insert(key.clone(), seq);
        }
        validate_levels(&event.bids, &event.asks, line_no + 1)?;
        let book = books.entry(key).or_default();
        if event.kind == EventKind::Snapshot {
            book.0.clear();
            book.1.clear();
            snapshots += 1;
        }
        apply(&mut book.0, &event.bids);
        apply(&mut book.1, &event.asks);
        if book.0.is_empty() || book.1.is_empty() {
            bail!("libro no reconstruible en línea {}", line_no + 1);
        }
        count += 1;
        gaps += u64::from(event.integrity.gap);
        rest += u64::from(matches!(event.source, TapeSource::Rest { .. }));
    }
    if count != manifest.events
        || snapshots != manifest.snapshots
        || gaps != manifest.sequence_gaps
        || rest != manifest.rest_fallback_events
    {
        bail!("conteos del manifiesto no coinciden con el tape");
    }
    if last_local.is_none_or(|v| v < manifest.started_at || v > manifest.ended_at) {
        bail!("ventana temporal del manifiesto no contiene los eventos");
    }
    Ok(Verification {
        path: path.to_path_buf(),
        events: count,
        books_reconstructed: books.len(),
        sha256: actual_sha,
    })
}

fn validate_levels(bids: &[NivelOrden], asks: &[NivelOrden], line: usize) -> anyhow::Result<()> {
    if bids.len() > 50
        || asks.len() > 50
        || bids.iter().chain(asks).any(|n| {
            !n.precio.is_finite() || !n.cantidad.is_finite() || n.precio <= 0.0 || n.cantidad < 0.0
        })
    {
        bail!("niveles inválidos en línea {line}");
    }
    if bids.windows(2).any(|w| w[0].precio < w[1].precio)
        || asks.windows(2).any(|w| w[0].precio > w[1].precio)
    {
        bail!("niveles desordenados en línea {line}");
    }
    Ok(())
}
fn apply(book: &mut BTreeMap<i64, f64>, levels: &[NivelOrden]) {
    for n in levels {
        let p = (n.precio * 100_000_000.0).round() as i64;
        if n.cantidad == 0.0 {
            book.remove(&p);
        } else {
            book.insert(p, n.cantidad);
        }
    }
}
fn hex_sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn file_sha(path: &Path) -> anyhow::Result<String> {
    Ok(hex_sha(&fs::read(path)?))
}
fn git_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

pub fn parse_duration(value: &str) -> anyhow::Result<Duration> {
    let split = value
        .find(|c: char| !c.is_ascii_digit())
        .context("duración inválida (ej. 6h, 30m, 10s)")?;
    let n: u64 = value[..split].parse()?;
    let unit = &value[split..];
    match unit {
        "h" => Ok(Duration::from_secs(n * 3600)),
        "m" => Ok(Duration::from_secs(n * 60)),
        "s" => Ok(Duration::from_secs(n)),
        _ => bail!("unidad de duración inválida: {unit}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duration_units() {
        assert_eq!(parse_duration("6h").unwrap(), Duration::from_secs(21600));
        assert!(parse_duration("6x").is_err());
    }
}
