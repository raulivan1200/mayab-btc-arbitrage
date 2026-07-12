//! Ledger JSONL encadenado para ejecuciones aisladas.

use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    path::Path,
};

use anyhow::{bail, Context};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerEntry {
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub event: String,
    pub payload: serde_json::Value,
    pub previous_hash: String,
    pub hash: String,
}

pub struct LedgerWriter {
    file: File,
    sequence: u64,
    previous_hash: String,
}

impl LedgerWriter {
    pub fn create(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self {
            file: File::create(path)
                .with_context(|| format!("no se pudo crear {}", path.display()))?,
            sequence: 0,
            previous_hash: "GENESIS".into(),
        })
    }

    pub fn append(
        &mut self,
        event: impl Into<String>,
        payload: serde_json::Value,
    ) -> anyhow::Result<()> {
        self.sequence += 1;
        let timestamp = Utc::now();
        let event = event.into();
        let hash = calculate_hash(
            self.sequence,
            timestamp,
            &event,
            &payload,
            &self.previous_hash,
        )?;
        let entry = LedgerEntry {
            sequence: self.sequence,
            timestamp,
            event,
            payload,
            previous_hash: self.previous_hash.clone(),
            hash: hash.clone(),
        };
        serde_json::to_writer(&mut self.file, &entry)?;
        self.file.write_all(b"\n")?;
        self.file.sync_data()?;
        self.previous_hash = hash;
        Ok(())
    }
}

pub fn independent_audit(path: impl AsRef<Path>) -> anyhow::Result<usize> {
    let file = File::open(path.as_ref())?;
    let mut expected_sequence = 1;
    let mut previous = "GENESIS".to_string();
    for (line_number, line) in BufReader::new(file).lines().enumerate() {
        let entry: LedgerEntry = serde_json::from_str(&line?)
            .with_context(|| format!("entrada inválida en línea {}", line_number + 1))?;
        if entry.sequence != expected_sequence || entry.previous_hash != previous {
            bail!("cadena inválida en secuencia {}", entry.sequence);
        }
        let expected = calculate_hash(
            entry.sequence,
            entry.timestamp,
            &entry.event,
            &entry.payload,
            &entry.previous_hash,
        )?;
        if entry.hash != expected {
            bail!("hash inválido en secuencia {}", entry.sequence);
        }
        previous = entry.hash;
        expected_sequence += 1;
    }
    Ok((expected_sequence - 1) as usize)
}

fn calculate_hash(
    sequence: u64,
    timestamp: DateTime<Utc>,
    event: &str,
    payload: &serde_json::Value,
    previous: &str,
) -> anyhow::Result<String> {
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(&(
        sequence, timestamp, event, payload, previous,
    ))?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auditor_detecta_ledger_valido() {
        let path = std::env::temp_dir().join(format!("mayab-ledger-{}.jsonl", std::process::id()));
        let mut ledger = LedgerWriter::create(&path).unwrap();
        ledger
            .append("preflight", serde_json::json!({"ok": true}))
            .unwrap();
        drop(ledger);
        assert_eq!(independent_audit(&path).unwrap(), 1);
        let _ = std::fs::remove_file(path);
    }
}
