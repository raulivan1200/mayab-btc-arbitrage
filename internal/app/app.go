package app

import (
	"context"
	"log/slog"

	"github.com/raulivan1200/mayab-btc-arbitrage/internal/config"
	"github.com/raulivan1200/mayab-btc-arbitrage/internal/mercado"
	"github.com/raulivan1200/mayab-btc-arbitrage/internal/motor"
)

type App struct {
	Config config.Config
	Motor  *motor.Motor
}

func Nueva(cfg config.Config, logger *slog.Logger) *App {
	fuentes := []motor.FuenteMercado{
		mercado.NuevoBinance(logger),
		mercado.NuevoKraken(logger),
		mercado.NuevoCoinbase(logger),
		mercado.NuevoOKX(logger),
		mercado.NuevoBybit(logger),
	}

	nombres := make([]string, 0, len(fuentes))
	for _, fuente := range fuentes {
		nombres = append(nombres, fuente.Nombre())
	}

	carteras := motor.NuevaCarteras(nombres, cfg.CapitalInicialUSD, cfg.BalanceInicialBTC)
	return &App{
		Config: cfg,
		Motor:  motor.Nuevo(cfg.Costos, carteras, fuentes, logger),
	}
}

func (a *App) Iniciar(ctx context.Context) {
	a.Motor.Iniciar(ctx, a.Config.IntervaloAnalisis)
}
