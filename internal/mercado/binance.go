package mercado

import (
	"encoding/json"
	"log/slog"

	"github.com/raulivan1200/mayab-btc-arbitrage/internal/motor"
)

func NuevoBinance(logger *slog.Logger) Adaptador {
	return Adaptador{
		NombreExchange: "Binance",
		URL:            "wss://data-stream.binance.vision/ws/btcusdt@bookTicker",
		Parsear:        parsearBinance,
		Logger:         logger,
	}
}

func parsearBinance(mensaje []byte) (motor.Cotizacion, bool) {
	var dato struct {
		Simbolo string `json:"s"`
		Bid     string `json:"b"`
		BidQty  string `json:"B"`
		Ask     string `json:"a"`
		AskQty  string `json:"A"`
	}
	if err := json.Unmarshal(mensaje, &dato); err != nil || dato.Bid == "" || dato.Ask == "" {
		return motor.Cotizacion{}, false
	}
	bid, okBid := decimalObligatorio(dato.Bid)
	ask, okAsk := decimalObligatorio(dato.Ask)
	if !okBid || !okAsk {
		return motor.Cotizacion{}, false
	}
	return motor.Cotizacion{
		Par:         "BTC/USDT",
		Bid:         bid,
		BidCantidad: decimalOpcional(dato.BidQty),
		Ask:         ask,
		AskCantidad: decimalOpcional(dato.AskQty),
	}, true
}
