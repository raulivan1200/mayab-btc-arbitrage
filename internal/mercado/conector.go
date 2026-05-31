package mercado

import (
	"context"
	"encoding/json"
	"log/slog"
	"math/rand/v2"
	"net/http"
	"time"

	"github.com/gorilla/websocket"
	"github.com/raulivan1200/mayab-btc-arbitrage/internal/motor"
)

type Adaptador struct {
	NombreExchange string
	URL            string
	Headers        http.Header
	Suscripcion    any
	Parsear        func([]byte) (motor.Cotizacion, bool)
	Logger         *slog.Logger
}

func (a Adaptador) Nombre() string {
	return a.NombreExchange
}

func (a Adaptador) Iniciar(ctx context.Context, salida chan<- motor.Cotizacion) {
	backoff := 650 * time.Millisecond
	for {
		select {
		case <-ctx.Done():
			return
		default:
		}

		err := a.conectar(ctx, salida)
		if err != nil && a.Logger != nil {
			a.Logger.Warn("feed desconectado", "exchange", a.NombreExchange, "error", err)
		}

		pausa := backoff + time.Duration(rand.Int64N(int64(backoff/2)+1))
		select {
		case <-ctx.Done():
			return
		case <-time.After(pausa):
		}
		if backoff < 8*time.Second {
			backoff *= 2
		}
	}
}

func (a Adaptador) conectar(ctx context.Context, salida chan<- motor.Cotizacion) error {
	dialer := websocket.Dialer{
		HandshakeTimeout: 8 * time.Second,
		Proxy:            http.ProxyFromEnvironment,
	}
	conn, _, err := dialer.DialContext(ctx, a.URL, a.Headers)
	if err != nil {
		return err
	}
	defer conn.Close()

	if a.Logger != nil {
		a.Logger.Info("feed conectado", "exchange", a.NombreExchange)
	}

	if a.Suscripcion != nil {
		payload, err := json.Marshal(a.Suscripcion)
		if err != nil {
			return err
		}
		if err := conn.WriteMessage(websocket.TextMessage, payload); err != nil {
			return err
		}
	}

	conn.SetReadLimit(1 << 20)
	if err := conn.SetReadDeadline(time.Now().Add(40 * time.Second)); err != nil {
		return err
	}
	conn.SetPongHandler(func(string) error {
		return conn.SetReadDeadline(time.Now().Add(40 * time.Second))
	})

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		_, mensaje, err := conn.ReadMessage()
		if err != nil {
			return err
		}
		if err := conn.SetReadDeadline(time.Now().Add(40 * time.Second)); err != nil {
			return err
		}
		cotizacion, ok := a.Parsear(mensaje)
		if !ok {
			continue
		}
		cotizacion.Exchange = a.NombreExchange
		cotizacion.RecibidaEn = time.Now()

		select {
		case salida <- cotizacion:
		case <-ctx.Done():
			return ctx.Err()
		}
	}
}
