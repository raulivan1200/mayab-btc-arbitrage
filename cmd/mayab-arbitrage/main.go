package main

import (
	"context"
	"log/slog"
	"os"
	"os/signal"
	"syscall"

	"github.com/raulivan1200/mayab-btc-arbitrage/internal/app"
	"github.com/raulivan1200/mayab-btc-arbitrage/internal/config"
	httpserver "github.com/raulivan1200/mayab-btc-arbitrage/internal/http"
)

func main() {
	logger := slog.New(slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelInfo}))
	ctx, detener := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer detener()

	cfg := config.CargarConLogger(logger)
	aplicacion := app.Nueva(cfg, logger)
	aplicacion.Iniciar(ctx)

	server := httpserver.Nuevo(aplicacion.Motor, logger)
	if err := httpserver.Escuchar(ctx, cfg.Port, server.Handler(), logger); err != nil {
		logger.Error("servidor detenido con error", "error", err)
		os.Exit(1)
	}
}
