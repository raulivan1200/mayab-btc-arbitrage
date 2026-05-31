package config

import (
	"log/slog"
	"os"
	"strconv"
	"strings"
	"time"

	"github.com/raulivan1200/mayab-btc-arbitrage/internal/motor"
)

type Config struct {
	Port              string
	ParBase           string
	IntervaloAnalisis time.Duration
	Costos            motor.MapaCostos
	CapitalInicialUSD float64
	BalanceInicialBTC float64
}

func Cargar() Config {
	return CargarConLogger(nil)
}

func CargarConLogger(logger *slog.Logger) Config {
	costos := motor.MapaCostos{
		MaxOperacionBTC:      envFloat(logger, "MAX_OPERACION_BTC", 0.18),
		MinUtilidadUSD:       envFloat(logger, "MIN_UTILIDAD_USD", 1.25),
		MinSpreadNetoBps:     envFloat(logger, "MIN_SPREAD_NETO_BPS", 0.65),
		SlippageBps:          envFloat(logger, "SLIPPAGE_BPS", 0.35),
		LatenciaRiesgoBps:    envFloat(logger, "LATENCIA_RIESGO_BPS", 0.08),
		RetiroAmortizadoBps:  envFloat(logger, "RETIRO_AMORTIZADO_BPS", 0.12),
		StaleMs:              envInt64(logger, "STALE_MS", 4500),
		CooldownMs:           envInt64(logger, "COOLDOWN_MS", 1400),
		Exchanges: map[string]motor.ExchangeConfig{
			"Binance": {
				Nombre:        "Binance",
				FeeTaker:      envFloat(logger, "FEE_BINANCE", 0.0010),
				RetiroBTC:     envFloat(logger, "RETIRO_BTC_BINANCE", 0.00010),
				Confiabilidad: 0.98,
			},
			"Kraken": {
				Nombre:        "Kraken",
				FeeTaker:      envFloat(logger, "FEE_KRAKEN", 0.0026),
				RetiroBTC:     envFloat(logger, "RETIRO_BTC_KRAKEN", 0.00020),
				Confiabilidad: 0.97,
			},
			"Coinbase": {
				Nombre:        "Coinbase",
				FeeTaker:      envFloat(logger, "FEE_COINBASE", 0.0060),
				RetiroBTC:     envFloat(logger, "RETIRO_BTC_COINBASE", 0.00012),
				Confiabilidad: 0.96,
			},
			"OKX": {
				Nombre:        "OKX",
				FeeTaker:      envFloat(logger, "FEE_OKX", 0.0010),
				RetiroBTC:     envFloat(logger, "RETIRO_BTC_OKX", 0.00010),
				Confiabilidad: 0.96,
			},
			"Bybit": {
				Nombre:        "Bybit",
				FeeTaker:      envFloat(logger, "FEE_BYBIT", 0.0010),
				RetiroBTC:     envFloat(logger, "RETIRO_BTC_BYBIT", 0.00010),
				Confiabilidad: 0.95,
			},
		},
	}

	return Config{
		Port:              env("PORT", "8080"),
		ParBase:           env("PAR_BASE", "BTC/USD"),
		IntervaloAnalisis: time.Duration(envInt64(logger, "INTERVALO_ANALISIS_MS", 70)) * time.Millisecond,
		Costos:            costos,
		CapitalInicialUSD: envFloat(logger, "CAPITAL_INICIAL_USD", 250000),
		BalanceInicialBTC: envFloat(logger, "BALANCE_INICIAL_BTC", 1.25),
	}
}

func env(clave string, fallback string) string {
	valor := strings.TrimSpace(os.Getenv(clave))
	if valor == "" {
		return fallback
	}
	return valor
}

func envFloat(logger *slog.Logger, clave string, fallback float64) float64 {
	valor := strings.TrimSpace(os.Getenv(clave))
	if valor == "" {
		return fallback
	}
	numero, err := strconv.ParseFloat(valor, 64)
	if err != nil {
		advertirFallback(logger, clave, valor, fallback, err)
		return fallback
	}
	return numero
}

func envInt64(logger *slog.Logger, clave string, fallback int64) int64 {
	valor := strings.TrimSpace(os.Getenv(clave))
	if valor == "" {
		return fallback
	}
	numero, err := strconv.ParseInt(valor, 10, 64)
	if err != nil {
		advertirFallback(logger, clave, valor, fallback, err)
		return fallback
	}
	return numero
}

func advertirFallback(logger *slog.Logger, clave string, valor string, fallback any, err error) {
	if logger == nil {
		return
	}
	logger.Warn(
		"variable de entorno inválida; usando valor por defecto",
		"clave", clave,
		"valor", valor,
		"valor_por_defecto", fallback,
		"error", err,
	)
}
