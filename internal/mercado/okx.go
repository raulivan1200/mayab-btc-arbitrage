package mercado

import (
	"encoding/json"
	"log/slog"

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
	bid, okBid := decimalObligatorio(item.BidPx)
	ask, okAsk := decimalObligatorio(item.AskPx)
	if !okBid || !okAsk {
		return motor.Cotizacion{}, false
	}
	return motor.Cotizacion{
		Par:          dato.Arg.InstID,
		Bid:          bid,
		BidCantidad:  decimalOpcional(item.BidSz),
		Ask:          ask,
		AskCantidad:  decimalOpcional(item.AskSz),
		EventoUnixMs: marcaTiempoUnixMs(item.TS),
	}, true
}
