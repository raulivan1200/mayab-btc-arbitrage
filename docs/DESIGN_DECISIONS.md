# Design decisions

- **Simulation first:** public feeds are real; execution, balances and P&L are simulated and labeled.
- **One binary:** engine, API and UI share a release artifact to simplify evaluation and local operation.
- **USD is not USDT:** lanes remain separate unless an explicit basis model enables crossing.
- **Net profitability:** decisions use fees, slippage, withdrawal amortization, latency and inventory—not headline spread.
- **Bounded observability:** metric labels come from static catalogs; market symbols and IDs stay out of labels.
- **Demo independent of market luck:** `mercado_rentable` is synthetic, repeatable and auditable.

Detailed historical context lives in `docs/ADRs/`.
