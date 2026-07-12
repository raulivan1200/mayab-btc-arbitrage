# Contributing

Thanks for improving Mayab. Keep every change compatible with its safety boundary: public market data and simulated execution only.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo run
```

Use a focused branch and describe the behavior, tests and user-facing contract in the pull request. Add unit tests in `motor.rs` when decision behavior changes. GA changes must work with real history and synthetic replay. Update UI and exports with any `EstadoPublico` or JSON change. Do not commit generated `target/`, databases, tokens, exchange credentials or personal market captures.

For a new adapter follow [Adding an exchange](docs/ADDING_EXCHANGE.md). For operations and smoke checks see [Operations](docs/OPERATIONS.md).

## Review checklist

- The change cannot place real orders, custody assets or transfer funds.
- `mercado_rentable` remains a truthful labeled synthetic demo.
- Errors and logs contain no token or secret.
- New metric labels have bounded cardinality.
- Documentation matches API/UI behavior.
- Formatting, lint, tests and relevant smoke checks pass.

By participating, you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).
