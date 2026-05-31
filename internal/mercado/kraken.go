package mercado

import (
	"encoding/json"
	"log/slog"

	"github.com/raulivan1200/mayab-btc-arbitrage/internal/motor"
)

func NuevoKraken(logger *slog.Logger) Adaptador {
	return Adaptador{
		NombreExchange: "Kraken",
		URL:            "wss://ws.kraken.com/v2",
		Suscripcion: map[string]any{
			"method": "subscribe",
			"params": map[string]any{
				"channel":       "ticker",
				"symbol":        []string{"BTC/USD"},
				"event_trigger": "bbo",
				"snapshot":      true,
			},
		},
		Parsear: parsearKraken,
		Logger:  logger,
	}
}

func parsearKraken(mensaje []byte) (motor.Cotizacion, bool) {
	var dato struct {
		Channel string `json:"channel"`
		Type    string `json:"type"`
		Data    []struct {
			Symbol    string  `json:"symbol"`
			Bid       float64 `json:"bid"`
			BidQty    float64 `json:"bid_qty"`
			Ask       float64 `json:"ask"`
			AskQty    float64 `json:"ask_qty"`
			Timestamp string  `json:"timestamp"`
		} `json:"data"`
	}
	if err := json.Unmarshal(mensaje, &dato); err != nil || dato.Channel != "ticker" || len(dato.Data) == 0 {
		return motor.Cotizacion{}, false
	}
	item := dato.Data[0]
	if item.Bid <= 0 || item.Ask <= 0 {
		return motor.Cotizacion{}, false
	}
	return motor.Cotizacion{
		Par:          item.Symbol,
		Bid:          item.Bid,
		BidCantidad:  item.BidQty,
		Ask:          item.Ask,
		AskCantidad:  item.AskQty,
		EventoUnixMs: marcaTiempoRFC3339Nano(item.Timestamp),
	}, true
}
