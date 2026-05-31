package motor

import (
	"context"
	"log/slog"
	"sort"
	"sync"
	"sync/atomic"
	"time"
)

type FuenteMercado interface {
	Nombre() string
	Iniciar(ctx context.Context, salida chan<- Cotizacion)
}

type Motor struct {
	costos           MapaCostos
	analizador       *Analizador
	carteras         *Carteras
	fuentes          []FuenteMercado
	inicio           time.Time
	mu               sync.RWMutex
	cotizaciones     map[string]Cotizacion
	oportunidades    []Oportunidad
	operaciones      []Operacion
	seriePnL         []PuntoSerie
	serieDiferencial []PuntoSerie
	enfriamiento     map[string]time.Time
	utilidad         float64
	eventos          atomic.Uint64
	opsDetectadas    atomic.Uint64
	opsEjecutadas    atomic.Uint64
	latenciaEWMA     float64
	logger           *slog.Logger
}

func Nuevo(costos MapaCostos, carteras *Carteras, fuentes []FuenteMercado, logger *slog.Logger) *Motor {
	return &Motor{
		costos:           costos,
		analizador:       NuevoAnalizador(costos),
		carteras:         carteras,
		fuentes:          fuentes,
		inicio:           time.Now(),
		cotizaciones:     make(map[string]Cotizacion),
		oportunidades:    make([]Oportunidad, 0, 128),
		operaciones:      make([]Operacion, 0, 128),
		seriePnL:         make([]PuntoSerie, 0, 256),
		serieDiferencial: make([]PuntoSerie, 0, 256),
		enfriamiento:     make(map[string]time.Time),
		logger:           logger,
	}
}

func (m *Motor) Iniciar(ctx context.Context, intervalo time.Duration) {
	canal := make(chan Cotizacion, 4096)
	for _, fuente := range m.fuentes {
		go fuente.Iniciar(ctx, canal)
	}

	ticker := time.NewTicker(intervalo)
	go func() {
		defer ticker.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case cotizacion := <-canal:
				m.recibirCotizacion(cotizacion)
			case ahora := <-ticker.C:
				m.analizar(ahora)
			}
		}
	}()
}

func (m *Motor) recibirCotizacion(c Cotizacion) {
	if c.RecibidaEn.IsZero() {
		c.RecibidaEn = time.Now()
	}
	if c.EventoUnixMs > 0 {
		c.LatenciaMs = max(time.Now().UnixMilli()-c.EventoUnixMs, 0)
	}
	c.Conectado = true
	c.Secuencia = m.eventos.Add(1)

	m.mu.Lock()
	m.cotizaciones[c.Exchange] = c
	if c.LatenciaMs > 0 {
		if m.latenciaEWMA == 0 {
			m.latenciaEWMA = float64(c.LatenciaMs)
		} else {
			m.latenciaEWMA = m.latenciaEWMA*0.88 + float64(c.LatenciaMs)*0.12
		}
	}
	m.mu.Unlock()
}

func (m *Motor) analizar(ahora time.Time) {
	cotizaciones := m.snapshotCotizaciones()
	oportunidades := m.analizador.Buscar(cotizaciones, m.carteras, ahora)
	if len(oportunidades) == 0 {
		return
	}

	m.opsDetectadas.Add(uint64(len(oportunidades)))
	mejorDiferencial := oportunidades[0].DiferencialNetoBps
	for _, oportunidad := range oportunidades {
		if oportunidad.DiferencialNetoBps > mejorDiferencial {
			mejorDiferencial = oportunidad.DiferencialNetoBps
		}
	}

	m.mu.Lock()
	m.oportunidades = append(oportunidades, m.oportunidades...)
	m.oportunidades = limitar(m.oportunidades, 80)
	m.serieDiferencial = limitarUltimos(append(m.serieDiferencial, PuntoSerie{Tiempo: ahora, Valor: mejorDiferencial}), 240)
	m.mu.Unlock()

	for _, oportunidad := range oportunidades {
		if oportunidad.Ejecutable && m.puedeEjecutar(oportunidad, ahora) {
			m.ejecutar(oportunidad, ahora)
			break
		}
	}
}

func (m *Motor) puedeEjecutar(o Oportunidad, ahora time.Time) bool {
	ruta := o.CompraEn + "->" + o.VentaEn
	m.mu.RLock()
	ultima := m.enfriamiento[ruta]
	m.mu.RUnlock()
	return ahora.Sub(ultima).Milliseconds() >= m.costos.EnfriamientoMs
}

func (m *Motor) ejecutar(o Oportunidad, ahora time.Time) {
	op := Operacion{
		ID:            o.ID,
		CompraEn:      o.CompraEn,
		VentaEn:       o.VentaEn,
		CantidadBTC:   o.CantidadBTC,
		PrecioCompra:  o.Ask,
		PrecioVenta:   o.Bid,
		UtilidadUSD:   o.UtilidadUSD,
		Costos:        o.Costos,
		Parcial:       o.Parcial,
		EjecutadaEn:   ahora,
		LatenciaMaxMs: o.LatenciaMaxMs,
	}
	m.carteras.AplicarOperacion(op)

	m.mu.Lock()
	defer m.mu.Unlock()

	m.operaciones = append([]Operacion{op}, m.operaciones...)
	m.operaciones = limitar(m.operaciones, 80)
	m.enfriamiento[o.CompraEn+"->"+o.VentaEn] = ahora
	m.utilidad += op.UtilidadUSD
	m.seriePnL = limitarUltimos(append(m.seriePnL, PuntoSerie{Tiempo: ahora, Valor: m.utilidad}), 240)
	m.opsEjecutadas.Add(1)
}

func (m *Motor) Estado() EstadoPublico {
	m.mu.RLock()
	defer m.mu.RUnlock()

	cotizaciones := make([]Cotizacion, 0, len(m.cotizaciones))
	for _, c := range m.cotizaciones {
		cotizaciones = append(cotizaciones, c)
	}
	sort.Slice(cotizaciones, func(i, j int) bool {
		return cotizaciones[i].Exchange < cotizaciones[j].Exchange
	})

	precio := precioReferencia(cotizaciones)
	capitalInicial := m.carteras.CapitalInicialUSD(precio)
	capitalActual := m.carteras.CapitalActualUSD(precio)
	retorno := 0.0
	if capitalInicial > 0 {
		retorno = (capitalActual - capitalInicial) / capitalInicial * 10000
	}

	return EstadoPublico{
		GeneradoEn:       time.Now(),
		Cotizaciones:     cotizaciones,
		Oportunidades:    append([]Oportunidad{}, m.oportunidades...),
		Operaciones:      append([]Operacion{}, m.operaciones...),
		Balances:         m.carteras.Snapshot(),
		SeriePnL:         append([]PuntoSerie{}, m.seriePnL...),
		SerieDiferencial: append([]PuntoSerie{}, m.serieDiferencial...),
		Metricas: Metricas{
			UptimeSegundos:       int64(time.Since(m.inicio).Seconds()),
			EventosMercado:       m.eventos.Load(),
			Oportunidades:        m.opsDetectadas.Load(),
			Operaciones:          m.opsEjecutadas.Load(),
			UtilidadAcumuladaUSD: m.utilidad,
			CapitalInicialUSD:    capitalInicial,
			CapitalActualUSD:     capitalActual,
			RetornoBps:           retorno,
			LatenciaPromedioMs:   m.latenciaEWMA,
			EstadoRiesgo:         estadoRiesgo(m.latenciaEWMA, m.costos.StaleMs),
			Trabajadores:         len(m.fuentes) + 1,
		},
		Configuracion: m.costos,
	}
}

func (m *Motor) snapshotCotizaciones() map[string]Cotizacion {
	m.mu.RLock()
	defer m.mu.RUnlock()

	snapshot := make(map[string]Cotizacion, len(m.cotizaciones))
	for k, v := range m.cotizaciones {
		snapshot[k] = v
	}
	return snapshot
}

func precioReferencia(cotizaciones []Cotizacion) float64 {
	total := 0.0
	contador := 0.0
	for _, c := range cotizaciones {
		if c.Bid > 0 && c.Ask > 0 {
			total += (c.Bid + c.Ask) / 2
			contador++
		}
	}
	if contador == 0 {
		return 100000
	}
	return total / contador
}

func estadoRiesgo(latencia float64, staleMs int64) string {
	if latencia == 0 {
		return "esperando mercado"
	}
	if latencia > float64(staleMs)*0.75 {
		return "latencia alta"
	}
	return "estable"
}

func limitar[T any](items []T, maximo int) []T {
	if len(items) <= maximo {
		return items
	}
	return items[:maximo]
}

func limitarUltimos[T any](items []T, maximo int) []T {
	if len(items) <= maximo {
		return items
	}
	return items[len(items)-maximo:]
}
