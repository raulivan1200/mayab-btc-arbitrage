use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use mayab_arbitrage::evaluation::{evaluate_tape, EvaluationConfig, Split};

fn main() -> Result<()> {
    let mut tape = None;
    let mut output = None;
    let mut split = Split::default();
    let mut seed = 0_u64;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--tape" => tape = args.next().map(PathBuf::from),
            "--output" => output = args.next().map(PathBuf::from),
            "--split" => {
                let raw = args.next().context("--split requiere A,B,C")?;
                split = raw.parse()?;
            }
            "--seed" => {
                seed = args
                    .next()
                    .context("--seed requiere un entero")?
                    .parse()
                    .context("--seed no es un u64 válido")?;
            }
            "-h" | "--help" => {
                println!("evaluate-tape --tape DIR|FILE --split 50,20,30 --seed N --output DIR");
                return Ok(());
            }
            other => bail!("argumento desconocido: {other}"),
        }
    }
    let config = EvaluationConfig {
        tape: tape.context("falta --tape")?,
        output: output.context("falta --output")?,
        split,
        seed,
    };
    let paths = evaluate_tape(&config)?;
    println!("JSON: {}", paths.json.display());
    println!("CSV: {}", paths.csv.display());
    println!("Markdown: {}", paths.markdown.display());
    Ok(())
}
