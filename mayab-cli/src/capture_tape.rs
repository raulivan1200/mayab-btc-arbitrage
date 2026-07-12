use mayab_arbitrage::tape::{capture, parse_duration, CaptureConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let mut duration = None;
    let mut output = None;
    let mut pair = "BTC/USD".to_string();
    let mut depth = 10usize;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--duration" => {
                duration =
                    Some(parse_duration(&args.next().ok_or_else(|| {
                        anyhow::anyhow!("falta valor de --duration")
                    })?)?)
            }
            "--output" => {
                output = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("falta valor de --output"))?,
                ))
            }
            "--pair" => {
                pair = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("falta valor de --pair"))?
            }
            "--depth" => {
                depth = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("falta valor de --depth"))?
                    .parse()?
            }
            "--help" | "-h" => {
                println!("capture-tape --duration 6h --output DIR [--pair BTC/USD] [--depth 10]");
                return Ok(());
            }
            _ => anyhow::bail!("argumento desconocido: {arg}"),
        }
    }
    if !(10..=50).contains(&depth) {
        anyhow::bail!("--depth debe estar entre 10 y 50");
    }
    let config = CaptureConfig {
        schema_version: 1,
        pair,
        exchanges: vec![
            "Binance".into(),
            "Kraken".into(),
            "Coinbase".into(),
            "OKX".into(),
        ],
        depth,
    };
    let manifest = capture(
        &output.ok_or_else(|| anyhow::anyhow!("--output es obligatorio"))?,
        duration.ok_or_else(|| anyhow::anyhow!("--duration es obligatorio"))?,
        config,
    )
    .await?;
    println!("{}", serde_json::to_string_pretty(&manifest)?);
    Ok(())
}
