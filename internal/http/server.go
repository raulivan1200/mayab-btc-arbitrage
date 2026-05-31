package httpserver

import (
	"context"
	"encoding/json"
	"io/fs"
	"log/slog"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/gorilla/websocket"
	"github.com/raulivan1200/mayab-btc-arbitrage/internal/motor"
	"github.com/raulivan1200/mayab-btc-arbitrage/internal/webui"
)

type Server struct {
	motor  *motor.Motor
	logger *slog.Logger
}

func Nuevo(m *motor.Motor, logger *slog.Logger) *Server {
	return &Server{motor: m, logger: logger}
}

func (s *Server) Handler() http.Handler {
	mux := http.NewServeMux()
	archivos, err := fs.Sub(webui.Archivos, "web")
	if err != nil {
		s.registrar("no se pudo cargar la interfaz web embebida", err)
		mux.HandleFunc("/", func(w http.ResponseWriter, _ *http.Request) {
			http.Error(w, "interfaz web no disponible", http.StatusInternalServerError)
		})
	} else {
		mux.Handle("/", http.FileServer(http.FS(archivos)))
	}
	mux.HandleFunc("/healthz", s.healthz)
	mux.HandleFunc("/api/estado", s.estado)
	mux.HandleFunc("/tiempo-real", s.tiempoReal)
	return seguridad(mux)
}

func (s *Server) healthz(w http.ResponseWriter, _ *http.Request) {
	responderJSON(w, map[string]bool{"ok": true})
}

func (s *Server) estado(w http.ResponseWriter, _ *http.Request) {
	responderJSON(w, s.motor.Estado())
}

func (s *Server) tiempoReal(w http.ResponseWriter, r *http.Request) {
	upgrader := websocket.Upgrader{
		CheckOrigin: func(r *http.Request) bool {
			origen := r.Header.Get("Origin")
			if origen == "" {
				return true
			}
			urlOrigen, err := url.Parse(origen)
			return err == nil && strings.EqualFold(urlOrigen.Host, r.Host)
		},
	}
	conn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		s.logger.Warn("websocket cliente rechazado", "error", err)
		return
	}
	defer conn.Close()

	ticker := time.NewTicker(180 * time.Millisecond)
	defer ticker.Stop()

	for {
		select {
		case <-r.Context().Done():
			return
		case <-ticker.C:
			if err := conn.WriteJSON(s.motor.Estado()); err != nil {
				return
			}
		}
	}
}

func Escuchar(ctx context.Context, puerto string, handler http.Handler, logger *slog.Logger) error {
	server := &http.Server{
		Addr:              ":" + puerto,
		Handler:           handler,
		ReadHeaderTimeout: 5 * time.Second,
	}

	go func() {
		<-ctx.Done()
		apagarCtx, cancelar := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancelar()
		if err := server.Shutdown(apagarCtx); err != nil && logger != nil {
			logger.Warn("apagado del servidor incompleto", "error", err)
		}
	}()

	logger.Info("servidor iniciado", "url", "http://localhost:"+puerto)
	err := server.ListenAndServe()
	if err == http.ErrServerClosed {
		return nil
	}
	return err
}

func (s *Server) registrar(mensaje string, err error) {
	if s.logger == nil {
		return
	}
	s.logger.Error(mensaje, "error", err)
}

func responderJSON(w http.ResponseWriter, valor any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	encoder := json.NewEncoder(w)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(valor); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
	}
}

func seguridad(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("X-Content-Type-Options", "nosniff")
		w.Header().Set("Referrer-Policy", "strict-origin-when-cross-origin")
		w.Header().Set("Permissions-Policy", "camera=(), microphone=(), geolocation=()")
		next.ServeHTTP(w, r)
	})
}
