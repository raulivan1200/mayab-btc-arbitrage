package mercado

import (
	"encoding/json"
	"log/slog"
	"strconv"

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
	return motor.Cotizacion{
		Par:         "BTC/USDT",
		Bid:         numero(dato.Bid),
		BidCantidad: numero(dato.BidQty),
		Ask:         numero(dato.Ask),
		AskCantidad: numero(dato.AskQty),
	}, true
}

func numero(valor string) float64 {
	num, _ := strconv.ParseFloat(valor, 64)
	return num
}
