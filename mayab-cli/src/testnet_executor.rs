#![forbid(unsafe_code)]

use mayab_arbitrage::testnet::{run_cycle, CoinbaseSandboxTransport, TestnetConfig};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(false)
                .with_span_list(false),
        )
        .init();
    let config = TestnetConfig::from_env()?;
    let ledger_path = config.ledger_path.clone();
    let transport = CoinbaseSandboxTransport::new(config.clone())?;
    let audited = run_cycle(&transport, &config).await?;
    tracing::info!(entries = audited, ledger = %ledger_path.display(), "ciclo sandbox reconciliado y auditado");
    Ok(())
}
