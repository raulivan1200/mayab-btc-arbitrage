FROM golang:1.26-alpine AS build

WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build -trimpath -ldflags="-s -w" -o /bin/mayab-arbitrage ./cmd/mayab-arbitrage

FROM gcr.io/distroless/static-debian12:nonroot

COPY --from=build /bin/mayab-arbitrage /mayab-arbitrage
ENV PORT=8080
EXPOSE 8080
USER nonroot:nonroot
ENTRYPOINT ["/mayab-arbitrage"]
