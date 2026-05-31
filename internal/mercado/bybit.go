package mercado

import (
	"encoding/json"
	"log/slog"

	"github.com/raulivan1200/mayab-btc-arbitrage/internal/motor"
)

func NuevoBybit(logger *slog.Logger) Adaptador {
	return Adaptador{
		NombreExchange: "Bybit",
		URL:            "wss://stream.bybit.com/v5/public/spot",
		Suscripcion: map[string]any{
			"op":   "subscribe",
			"args": []string{"tickers.BTCUSDT"},
		},
		Parsear: parsearBybit,
		Logger:  logger,
	}
}

func parsearBybit(mensaje []byte) (motor.Cotizacion, bool) {
	var dato struct {
		Topic string `json:"topic"`
		TS    int64  `json:"ts"`
		Data  struct {
			Symbol   string `json:"symbol"`
			BidPrice string `json:"bid1Price"`
			BidSize  string `json:"bid1Size"`
			AskPrice string `json:"ask1Price"`
			AskSize  string `json:"ask1Size"`
		} `json:"data"`
	}
	if err := json.Unmarshal(mensaje, &dato); err != nil || dato.Topic != "tickers.BTCUSDT" {
		return motor.Cotizacion{}, false
	}
	if dato.Data.BidPrice == "" || dato.Data.AskPrice == "" {
		return motor.Cotizacion{}, false
	}
	return motor.Cotizacion{
		Par:          dato.Data.Symbol,
		Bid:          numero(dato.Data.BidPrice),
		BidCantidad:  numero(dato.Data.BidSize),
		Ask:          numero(dato.Data.AskPrice),
		AskCantidad:  numero(dato.Data.AskSize),
		EventoUnixMs: dato.TS,
	}, true
}
