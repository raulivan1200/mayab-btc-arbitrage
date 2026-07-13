use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use mayab_arbitrage::tape::{query_corpus_sqlite, CorpusIndexQuery};

fn main() -> Result<()> {
    let Some((database, query)) = parse_args()? else {
        return Ok(());
    };
    let page = query_corpus_sqlite(&database, &query)
        .with_context(|| format!("no se pudo consultar {}", database.display()))?;
    println!("{}", serde_json::to_string_pretty(&page)?);
    Ok(())
}

fn parse_args() -> Result<Option<(PathBuf, CorpusIndexQuery)>> {
    let mut database = None;
    let mut query = CorpusIndexQuery {
        limit: 100,
        ..CorpusIndexQuery::default()
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--database" => database = args.next().map(PathBuf::from),
            "--corpus" => query.corpus_sha256 = args.next(),
            "--exchange" => query.exchange = args.next(),
            "--pair" => query.pair = args.next(),
            "--from" => {
                query.started_at_or_after = Some(parse_timestamp(&next(&mut args, "--from")?)?)
            }
            "--to" => query.ended_at_or_before = Some(parse_timestamp(&next(&mut args, "--to")?)?),
            "--after-start" => {
                query.after_started_at = Some(parse_timestamp(&next(&mut args, "--after-start")?)?)
            }
            "--after-sha" => query.after_sha256 = Some(next(&mut args, "--after-sha")?),
            "--limit" => query.limit = next(&mut args, "--limit")?.parse()?,
            "-h" | "--help" => {
                println!(
                    "query-corpus --database corpus.sqlite [--corpus SHA] [--exchange NAME] \
[--pair BTC/USD] [--from RFC3339] [--to RFC3339] [--limit 100] \
[--after-start RFC3339 --after-sha SHA]"
                );
                return Ok(None);
            }
            other => bail!("argumento desconocido: {other}"),
        }
    }
    if query.limit == 0 || query.limit > 500 {
        bail!("--limit debe estar entre 1 y 500");
    }
    Ok(Some((database.context("falta --database")?, query)))
}

fn next(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    args.next()
        .with_context(|| format!("falta valor para {flag}"))
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("timestamp RFC3339 inválido: {value}"))?
        .with_timezone(&Utc))
}
