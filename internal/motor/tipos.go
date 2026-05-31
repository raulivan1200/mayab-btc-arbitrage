package motor

import "time"

type Cotizacion struct {
	Exchange      string    `json:"exchange"`
	Par           string    `json:"par"`
	Bid           float64   `json:"bid"`
	BidCantidad   float64   `json:"bidCantidad"`
	Ask           float64   `json:"ask"`
	AskCantidad   float64   `json:"askCantidad"`
	EventoUnixMs  int64     `json:"eventoUnixMs"`
	RecibidaEn    time.Time `json:"recibidaEn"`
	LatenciaMs    int64     `json:"latenciaMs"`
	Secuencia     uint64    `json:"secuencia"`
	Conectado     bool      `json:"conectado"`
	UltimoMensaje string    `json:"ultimoMensaje,omitempty"`
}

type ExchangeConfig struct {
	Nombre        string  `json:"nombre"`
	FeeTaker      float64 `json:"feeTaker"`
	RetiroBTC     float64 `json:"retiroBtc"`
	Confiabilidad float64 `json:"confiabilidad"`
}

type CostosOperacion struct {
	FeeCompraUSD      float64 `json:"feeCompraUsd"`
	FeeVentaUSD       float64 `json:"feeVentaUsd"`
	SlippageUSD       float64 `json:"slippageUsd"`
	RetiroAmortUSD    float64 `json:"retiroAmortUsd"`
	LatenciaRiesgoUSD float64 `json:"latenciaRiesgoUsd"`
	TotalUSD          float64 `json:"totalUsd"`
}

type Oportunidad struct {
	ID             string          `json:"id"`
	CompraEn       string          `json:"compraEn"`
	VentaEn        string          `json:"ventaEn"`
	Ask            float64         `json:"ask"`
	Bid            float64         `json:"bid"`
	SpreadBrutoUSD float64         `json:"spreadBrutoUsd"`
	SpreadBrutoBps float64         `json:"spreadBrutoBps"`
	SpreadNetoUSD  float64         `json:"spreadNetoUsd"`
	SpreadNetoBps  float64         `json:"spreadNetoBps"`
	CantidadBTC    float64         `json:"cantidadBtc"`
	UtilidadUSD    float64         `json:"utilidadUsd"`
	Costos         CostosOperacion `json:"costos"`
	LatenciaMaxMs  int64           `json:"latenciaMaxMs"`
	DetectadaEn    time.Time       `json:"detectadaEn"`
	Razon          string          `json:"razon"`
	Ejecutable     bool            `json:"ejecutable"`
	Parcial        bool            `json:"parcial"`
}

type Operacion struct {
	ID            string          `json:"id"`
	CompraEn      string          `json:"compraEn"`
	VentaEn       string          `json:"ventaEn"`
	CantidadBTC   float64         `json:"cantidadBtc"`
	PrecioCompra  float64         `json:"precioCompra"`
	PrecioVenta   float64         `json:"precioVenta"`
	UtilidadUSD   float64         `json:"utilidadUsd"`
	Costos        CostosOperacion `json:"costos"`
	Parcial       bool            `json:"parcial"`
	EjecutadaEn   time.Time       `json:"ejecutadaEn"`
	LatenciaMaxMs int64           `json:"latenciaMaxMs"`
}

type Balance struct {
	Exchange string  `json:"exchange"`
	USD      float64 `json:"usd"`
	BTC      float64 `json:"btc"`
}

type PuntoSerie struct {
	Tiempo time.Time `json:"tiempo"`
	Valor  float64   `json:"valor"`
}

type Metricas struct {
	UptimeSegundos       int64   `json:"uptimeSegundos"`
	EventosMercado       uint64  `json:"eventosMercado"`
	Oportunidades        uint64  `json:"oportunidades"`
	Operaciones          uint64  `json:"operaciones"`
	UtilidadAcumuladaUSD float64 `json:"utilidadAcumuladaUsd"`
	CapitalInicialUSD    float64 `json:"capitalInicialUsd"`
	CapitalActualUSD     float64 `json:"capitalActualUsd"`
	RetornoBps           float64 `json:"retornoBps"`
	LatenciaPromedioMs   float64 `json:"latenciaPromedioMs"`
	EstadoRiesgo         string  `json:"estadoRiesgo"`
	Trabajadores         int     `json:"trabajadores"`
}

type EstadoPublico struct {
	GeneradoEn    time.Time     `json:"generadoEn"`
	Cotizaciones  []Cotizacion  `json:"cotizaciones"`
	Oportunidades []Oportunidad `json:"oportunidades"`
	Operaciones   []Operacion   `json:"operaciones"`
	Balances      []Balance     `json:"balances"`
	SeriePnL      []PuntoSerie  `json:"seriePnl"`
	SerieSpread   []PuntoSerie  `json:"serieSpread"`
	Metricas      Metricas      `json:"metricas"`
	Configuracion MapaCostos    `json:"configuracion"`
}

type MapaCostos struct {
	MaxOperacionBTC     float64                   `json:"maxOperacionBtc"`
	MinUtilidadUSD      float64                   `json:"minUtilidadUsd"`
	MinSpreadNetoBps    float64                   `json:"minSpreadNetoBps"`
	SlippageBps         float64                   `json:"slippageBps"`
	LatenciaRiesgoBps   float64                   `json:"latenciaRiesgoBps"`
	RetiroAmortizadoBps float64                   `json:"retiroAmortizadoBps"`
	StaleMs             int64                     `json:"staleMs"`
	CooldownMs          int64                     `json:"cooldownMs"`
	Exchanges           map[string]ExchangeConfig `json:"exchanges"`
}
