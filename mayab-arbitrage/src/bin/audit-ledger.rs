use anyhow::Result;
#[path = "../pnl_audit.rs"]
mod ledger_audit;
use ledger_audit::audit_path;
use rust_decimal::Decimal;
use std::{env, process::ExitCode};

fn money(value: Decimal) -> String {
    format!("{value:.4}")
}

fn main() -> Result<ExitCode> {
    let path = env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("uso: audit-ledger <ledger.json>"))?;
    let r = audit_path(path)?;
    println!(
        "Reported net P&L:     {:>10} USD",
        money(r.reported_net_pnl_usd)
    );
    println!(
        "Recomputed net P&L:   {:>10} USD",
        money(r.recomputed_net_pnl_usd)
    );
    println!(
        "Difference:           {:>10} USD",
        money(r.recomputed_net_pnl_usd - r.reported_net_pnl_usd)
    );
    println!(
        "Ledger integrity:      {}",
        if r.ledger_integrity { "PASS" } else { "FAIL" }
    );
    println!(
        "Balance conservation:  {}",
        if r.balance_conservation {
            "PASS"
        } else {
            "FAIL"
        }
    );
    println!("Residual exposure:     {:.8} BTC", r.residual_exposure_btc);
    println!("Final balance venues:   {}", r.final_balances.len());
    println!(
        "Bought / sold notional: {} / {} USD",
        money(r.bought_notional_usd),
        money(r.sold_notional_usd)
    );
    println!(
        "Fees buy / sell:        {} / {} USD",
        money(r.buy_fees_usd),
        money(r.sell_fees_usd)
    );
    println!("Slippage:               {} USD", money(r.slippage_usd));
    println!(
        "Rebalance / withdrawal: {} / {} USD",
        money(r.rebalance_cost_usd),
        money(r.withdrawal_cost_usd)
    );
    println!(
        "Realized / unrealized:  {} / {} USD",
        money(r.realized_pnl_usd),
        money(r.unrealized_pnl_usd)
    );
    for error in &r.errors {
        eprintln!("- {error}");
    }
    Ok(if r.errors.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
