use std::path::PathBuf;
fn main() -> anyhow::Result<()> {
    let path = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("uso: verify-tape DIR"))?,
    );
    let result = mayab_arbitrage::tape::verify(&path)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
