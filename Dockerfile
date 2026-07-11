FROM rust:1.96-slim AS build

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo fetch --locked
COPY internal/webui/web ./internal/webui/web
COPY README.md ARCHITECTURE.md DEMO_SCRIPT.md ./
COPY scripts ./scripts
RUN cargo build --release --locked

FROM gcr.io/distroless/cc-debian12:nonroot

WORKDIR /app
COPY --from=build /src/target/release/mayab-arbitrage /mayab-arbitrage
COPY --from=build /src/internal/webui/web /app/internal/webui/web
COPY README.md /app/README.md
COPY --from=build /src/ARCHITECTURE.md /app/ARCHITECTURE.md
COPY --from=build /src/DEMO_SCRIPT.md /app/DEMO_SCRIPT.md
COPY --from=build /src/scripts /app/scripts
ENV PORT=8080
ENV RUST_LOG=error
EXPOSE 8080
USER nonroot:nonroot
ENTRYPOINT ["/mayab-arbitrage"]
