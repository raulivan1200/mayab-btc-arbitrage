FROM rust:1.96-slim AS build

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch --locked
COPY src ./src
COPY internal/webui/web ./internal/webui/web
RUN cargo build --release --locked

FROM gcr.io/distroless/cc-debian12:nonroot

WORKDIR /app
COPY --from=build /src/target/release/mayab-arbitrage /mayab-arbitrage
COPY --from=build /src/internal/webui/web /app/internal/webui/web
ENV PORT=8080
ENV RUST_LOG=error
EXPOSE 8080
USER nonroot:nonroot
ENTRYPOINT ["/mayab-arbitrage"]
