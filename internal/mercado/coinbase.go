package mercado

import (
	"encoding/json"
	"log/slog"
	"strconv"
	"time"

	"github.com/raulivan1200/mayab-btc-arbitrage/internal/motor"
)

func NuevoCoinbase(logger *slog.Logger) Adaptador {
	return Adaptador{
		NombreExchange: "Coinbase",
		URL:            "wss://advanced-trade-ws.coinbase.com",
		Suscripcion: map[string]any{
			"type":        "subscribe",
			"product_ids": []string{"BTC-USD"},
			"channel":     "ticker",
		},
		Parsear: parsearCoinbase,
		Logger:  logger,
	}
}

func parsearCoinbase(mensaje []byte) (motor.Cotizacion, bool) {
	var dato struct {
		Channel   string `json:"channel"`
		Timestamp string `json:"timestamp"`
		Events    []struct {
			Type    string `json:"type"`
			Tickers []struct {
				ProductID   string `json:"product_id"`
				BestBid     string `json:"best_bid"`
				BestBidQty  string `json:"best_bid_quantity"`
				BestAsk     string `json:"best_ask"`
				BestAskQty  string `json:"best_ask_quantity"`
				BestBidSize string `json:"best_bid_size"`
				BestAskSize string `json:"best_ask_size"`
				Price       string `json:"price"`
			} `json:"tickers"`
		} `json:"events"`
	}
	if err := json.Unmarshal(mensaje, &dato); err != nil || dato.Channel != "ticker" || len(dato.Events) == 0 {
		return motor.Cotizacion{}, false
	}
	for _, evento := range dato.Events {
		for _, ticker := range evento.Tickers {
			bid := numeroCoinbase(ticker.BestBid)
			ask := numeroCoinbase(ticker.BestAsk)
			if bid <= 0 || ask <= 0 {
				continue
			}
			bidQty := numeroCoinbase(ticker.BestBidQty)
			if bidQty == 0 {
				bidQty = numeroCoinbase(ticker.BestBidSize)
			}
			askQty := numeroCoinbase(ticker.BestAskQty)
			if askQty == 0 {
				askQty = numeroCoinbase(ticker.BestAskSize)
			}
			eventoMs := int64(0)
			if dato.Timestamp != "" {
				if ts, err := time.Parse(time.RFC3339Nano, dato.Timestamp); err == nil {
					eventoMs = ts.UnixMilli()
				}
			}
			return motor.Cotizacion{
				Par:          ticker.ProductID,
				Bid:          bid,
				BidCantidad:  bidQty,
				Ask:          ask,
				AskCantidad:  askQty,
				EventoUnixMs: eventoMs,
			}, true
		}
	}
	return motor.Cotizacion{}, false
}

func numeroCoinbase(valor string) float64 {
	num, _ := strconv.ParseFloat(valor, 64)
	return num
}
