package mercado

import (
	"encoding/json"
	"log/slog"
	"strconv"

	"github.com/raulivan1200/mayab-btc-arbitrage/internal/motor"
)

func NuevoOKX(logger *slog.Logger) Adaptador {
	return Adaptador{
		NombreExchange: "OKX",
		URL:            "wss://ws.okx.com:8443/ws/v5/public",
		Suscripcion: map[string]any{
			"op": "subscribe",
			"args": []map[string]string{
				{"channel": "tickers", "instId": "BTC-USDT"},
			},
		},
		Parsear: parsearOKX,
		Logger:  logger,
	}
}

func parsearOKX(mensaje []byte) (motor.Cotizacion, bool) {
	var dato struct {
		Arg struct {
			Channel string `json:"channel"`
			InstID  string `json:"instId"`
		} `json:"arg"`
		Data []struct {
			BidPx string `json:"bidPx"`
			BidSz string `json:"bidSz"`
			AskPx string `json:"askPx"`
			AskSz string `json:"askSz"`
			TS    string `json:"ts"`
		} `json:"data"`
	}
	if err := json.Unmarshal(mensaje, &dato); err != nil || dato.Arg.Channel != "tickers" || len(dato.Data) == 0 {
		return motor.Cotizacion{}, false
	}
	item := dato.Data[0]
	eventoMs, _ := strconv.ParseInt(item.TS, 10, 64)
	return motor.Cotizacion{
		Par:          dato.Arg.InstID,
		Bid:          numero(item.BidPx),
		BidCantidad:  numero(item.BidSz),
		Ask:          numero(item.AskPx),
		AskCantidad:  numero(item.AskSz),
		EventoUnixMs: eventoMs,
	}, true
}
