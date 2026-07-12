//! Auditor P&L autocontenido; no enlaza ni importa el motor de decisión.
use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub btc: Decimal,
    pub usd: Decimal,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ledger {
    pub version: u32,
    pub run_id: String,
    pub initial_balances: BTreeMap<String, Balance>,
    pub events: Vec<Event>,
    pub final_balances: BTreeMap<String, Balance>,
    pub mark_price_usd: Decimal,
    pub reported_net_pnl_usd: Decimal,
    pub event_count: usize,
    pub head_hash: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub sequence: u64,
    pub previous_hash: String,
    pub hash: String,
    #[serde(flatten)]
    pub data: EventData,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventData {
    Fill {
        fill_id: String,
        exchange: String,
        side: Side,
        quantity_btc: Decimal,
        execution_price_usd: Decimal,
        reference_price_usd: Decimal,
        fee_usd: Decimal,
    },
    Rebalance {
        from_exchange: String,
        to_exchange: String,
        btc: Decimal,
        usd: Decimal,
        cost_usd: Decimal,
    },
    WithdrawalAmortization {
        exchange: String,
        cost_usd: Decimal,
    },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Buy,
    Sell,
}
#[derive(Debug)]
pub struct AuditReport {
    pub bought_notional_usd: Decimal,
    pub sold_notional_usd: Decimal,
    pub buy_fees_usd: Decimal,
    pub sell_fees_usd: Decimal,
    pub slippage_usd: Decimal,
    pub rebalance_cost_usd: Decimal,
    pub withdrawal_cost_usd: Decimal,
    pub realized_pnl_usd: Decimal,
    pub unrealized_pnl_usd: Decimal,
    pub recomputed_net_pnl_usd: Decimal,
    pub reported_net_pnl_usd: Decimal,
    pub residual_exposure_btc: Decimal,
    pub final_balances: BTreeMap<String, Balance>,
    pub ledger_integrity: bool,
    pub balance_conservation: bool,
    pub errors: Vec<String>,
}

#[derive(Serialize)]
struct HashInput<'a> {
    id: &'a str,
    sequence: u64,
    previous_hash: &'a str,
    data: &'a EventData,
}
pub fn hash(e: &Event) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&HashInput {
            id: &e.id,
            sequence: e.sequence,
            previous_hash: &e.previous_hash,
            data: &e.data
        })?)
    ))
}
pub fn seal(events: &mut [Event]) -> Result<String> {
    let mut p = "GENESIS".to_string();
    for (i, e) in events.iter_mut().enumerate() {
        e.sequence = i as u64 + 1;
        e.previous_hash = p;
        e.hash = hash(e)?;
        p = e.hash.clone();
    }
    Ok(p)
}
pub fn audit_path(path: impl AsRef<Path>) -> Result<AuditReport> {
    let b = fs::read(path.as_ref())
        .with_context(|| format!("no se pudo leer {}", path.as_ref().display()))?;
    audit(&serde_json::from_slice(&b).context("ledger JSON inválido")?)
}
pub fn audit(l: &Ledger) -> Result<AuditReport> {
    let mut errors = Vec::new();
    let mut ids = HashSet::new();
    let mut fills = HashSet::new();
    let mut prev = "GENESIS".to_string();
    for (i, e) in l.events.iter().enumerate() {
        if !ids.insert(&e.id) {
            errors.push(format!("ID duplicado: {}", e.id));
        }
        if e.sequence != i as u64 + 1 {
            errors.push(format!("secuencia inválida: {}", e.id));
        }
        if e.previous_hash != prev {
            errors.push(format!("cadena rota: {}", e.id));
        }
        match hash(e) {
            Ok(h) if h == e.hash => prev = h,
            _ => errors.push(format!("hash inválido: {}", e.id)),
        }
        if let EventData::Fill { fill_id, .. } = &e.data {
            if !fills.insert(fill_id) {
                errors.push(format!("fill aplicado dos veces: {fill_id}"));
            }
        }
    }
    if l.event_count != l.events.len() {
        errors.push("conteo de eventos no coincide".into())
    }
    if l.head_hash != prev {
        errors.push("head hash no coincide".into())
    }
    let mut bal = l.initial_balances.clone();
    let (ib, iu) = totals(&bal);
    let (mut bn, mut sn, mut bf, mut sf, mut slip, mut rc, mut wc, mut inv, mut cost, mut realized) = (
        Decimal::ZERO,
        Decimal::ZERO,
        Decimal::ZERO,
        Decimal::ZERO,
        Decimal::ZERO,
        Decimal::ZERO,
        Decimal::ZERO,
        Decimal::ZERO,
        Decimal::ZERO,
        Decimal::ZERO,
    );
    for e in &l.events {
        match &e.data {
            EventData::Fill {
                exchange,
                side,
                quantity_btc: q,
                execution_price_usd: p,
                reference_price_usd: r,
                fee_usd: f,
                ..
            } => {
                if *q <= Decimal::ZERO || *p <= Decimal::ZERO || *f < Decimal::ZERO {
                    errors.push(format!("importe inválido: {}", e.id));
                    continue;
                }
                let n = *q * *p;
                let b = bal.entry(exchange.clone()).or_insert(Balance {
                    btc: Decimal::ZERO,
                    usd: Decimal::ZERO,
                });
                match side {
                    Side::Buy => {
                        bn += n;
                        bf += *f;
                        slip += (*p - *r) * *q;
                        b.btc += *q;
                        b.usd -= n + *f;
                        inv += *q;
                        cost += n + *f
                    }
                    Side::Sell => {
                        sn += n;
                        sf += *f;
                        slip += (*r - *p) * *q;
                        b.btc -= *q;
                        b.usd += n - *f;
                        if inv < *q {
                            errors.push(format!("venta sin inventario: {}", e.id))
                        } else {
                            let c = cost * *q / inv;
                            realized += n - *f - c;
                            inv -= *q;
                            cost -= c
                        }
                    }
                }
            }
            EventData::Rebalance {
                from_exchange,
                to_exchange,
                btc,
                usd,
                cost_usd,
            } => {
                rc += *cost_usd;
                let b = bal.entry(from_exchange.clone()).or_insert(Balance {
                    btc: Decimal::ZERO,
                    usd: Decimal::ZERO,
                });
                b.btc -= *btc;
                b.usd -= *usd + *cost_usd;
                let b = bal.entry(to_exchange.clone()).or_insert(Balance {
                    btc: Decimal::ZERO,
                    usd: Decimal::ZERO,
                });
                b.btc += *btc;
                b.usd += *usd
            }
            EventData::WithdrawalAmortization { exchange, cost_usd } => {
                wc += *cost_usd;
                bal.entry(exchange.clone())
                    .or_insert(Balance {
                        btc: Decimal::ZERO,
                        usd: Decimal::ZERO,
                    })
                    .usd -= *cost_usd
            }
        }
        for (x, b) in &bal {
            if b.btc < Decimal::ZERO || b.usd < Decimal::ZERO {
                errors.push(format!("saldo negativo en {x} después de {}", e.id))
            }
        }
    }
    let (fb, fu) = totals(&bal);
    let residual = fb - ib;
    let recomputed = fu - iu + residual * l.mark_price_usd;
    let unrealized = if inv > Decimal::ZERO {
        inv * l.mark_price_usd - cost
    } else {
        Decimal::ZERO
    };
    let conservation = bal == l.final_balances;
    if !conservation {
        errors.push("saldos finales no coinciden".into())
    }
    if recomputed != l.reported_net_pnl_usd {
        errors.push("P&L reportado no coincide".into())
    }
    let integrity = !errors.iter().any(|e| {
        [
            "ID duplicado",
            "secuencia",
            "cadena",
            "hash",
            "dos veces",
            "conteo",
            "head hash",
        ]
        .iter()
        .any(|x| e.contains(x))
    });
    let balance_ok = conservation && !errors.iter().any(|e| e.contains("saldo negativo"));
    Ok(AuditReport {
        bought_notional_usd: bn,
        sold_notional_usd: sn,
        buy_fees_usd: bf,
        sell_fees_usd: sf,
        slippage_usd: slip,
        rebalance_cost_usd: rc,
        withdrawal_cost_usd: wc,
        realized_pnl_usd: realized,
        unrealized_pnl_usd: unrealized,
        recomputed_net_pnl_usd: recomputed,
        reported_net_pnl_usd: l.reported_net_pnl_usd,
        residual_exposure_btc: residual,
        final_balances: bal,
        ledger_integrity: integrity,
        balance_conservation: balance_ok,
        errors,
    })
}
fn totals(b: &BTreeMap<String, Balance>) -> (Decimal, Decimal) {
    b.values()
        .fold((Decimal::ZERO, Decimal::ZERO), |(x, y), v| {
            (x + v.btc, y + v.usd)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }
    fn ev(id: &str, data: EventData) -> Event {
        Event {
            id: id.into(),
            sequence: 0,
            previous_hash: String::new(),
            hash: String::new(),
            data,
        }
    }
    fn good() -> Ledger {
        let initial = BTreeMap::from([
            (
                "a".into(),
                Balance {
                    btc: d("1"),
                    usd: d("100000"),
                },
            ),
            (
                "b".into(),
                Balance {
                    btc: d("1"),
                    usd: d("100000"),
                },
            ),
        ]);
        let mut events = vec![
            ev(
                "e1",
                EventData::Fill {
                    fill_id: "f1".into(),
                    exchange: "a".into(),
                    side: Side::Buy,
                    quantity_btc: d("1"),
                    execution_price_usd: d("50000"),
                    reference_price_usd: d("49998"),
                    fee_usd: d("5"),
                },
            ),
            ev(
                "e2",
                EventData::Fill {
                    fill_id: "f2".into(),
                    exchange: "b".into(),
                    side: Side::Sell,
                    quantity_btc: d("1"),
                    execution_price_usd: d("50100"),
                    reference_price_usd: d("50103"),
                    fee_usd: d("5"),
                },
            ),
            ev(
                "e3",
                EventData::Rebalance {
                    from_exchange: "b".into(),
                    to_exchange: "a".into(),
                    btc: d("0"),
                    usd: d("10000"),
                    cost_usd: d("4"),
                },
            ),
            ev(
                "e4",
                EventData::WithdrawalAmortization {
                    exchange: "a".into(),
                    cost_usd: d("4.5769"),
                },
            ),
        ];
        let head = seal(&mut events).unwrap();
        let mut l = Ledger {
            version: 1,
            run_id: "run-001".into(),
            initial_balances: initial,
            events,
            final_balances: BTreeMap::new(),
            mark_price_usd: d("50050"),
            reported_net_pnl_usd: d("81.4231"),
            event_count: 4,
            head_hash: head,
        };
        l.final_balances = audit(&l).unwrap().final_balances;
        l
    }
    #[test]
    fn valid() {
        assert!(audit(&good()).unwrap().errors.is_empty())
    }
    #[test]
    fn deleted() {
        let mut l = good();
        l.events.remove(1);
        assert!(!audit(&l).unwrap().ledger_integrity)
    }
    #[test]
    fn fee_changed() {
        let mut l = good();
        if let EventData::Fill { fee_usd, .. } = &mut l.events[0].data {
            *fee_usd += d("1")
        }
        assert!(audit(&l)
            .unwrap()
            .errors
            .iter()
            .any(|e| e.contains("hash inválido")))
    }
    #[test]
    fn fill_twice() {
        let mut l = good();
        let mut e = l.events[0].clone();
        e.id = "e5".into();
        l.events.push(e);
        l.event_count = 5;
        l.head_hash = seal(&mut l.events).unwrap();
        assert!(audit(&l)
            .unwrap()
            .errors
            .iter()
            .any(|e| e.contains("dos veces")))
    }
    #[test]
    fn negative() {
        let mut l = good();
        if let EventData::Fill { quantity_btc, .. } = &mut l.events[1].data {
            *quantity_btc = d("3")
        }
        l.head_hash = seal(&mut l.events).unwrap();
        assert!(audit(&l)
            .unwrap()
            .errors
            .iter()
            .any(|e| e.contains("saldo negativo")))
    }
    #[test]
    fn duplicate_id() {
        let mut l = good();
        l.events[1].id = l.events[0].id.clone();
        l.head_hash = seal(&mut l.events).unwrap();
        assert!(audit(&l)
            .unwrap()
            .errors
            .iter()
            .any(|e| e.contains("ID duplicado")))
    }
    #[test]
    fn pnl_changed() {
        let mut l = good();
        l.reported_net_pnl_usd += d("1");
        assert!(audit(&l)
            .unwrap()
            .errors
            .iter()
            .any(|e| e.contains("P&L reportado")))
    }
}
