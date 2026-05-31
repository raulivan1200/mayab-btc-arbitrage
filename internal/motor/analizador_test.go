package motor

import (
	"testing"
	"time"
)

func TestAnalizadorEjecutaSoloConUtilidadNeta(t *testing.T) {
	costos := MapaCostos{
		MaxOperacionBTC:       0.5,
		MinUtilidadUSD:        1,
		MinDiferencialNetoBps: 0.1,
		DeslizamientoBps:      0.1,
		LatenciaRiesgoBps:     0.01,
		RetiroAmortizadoBps:   0.01,
		StaleMs:               1000,
		Exchanges: map[string]ExchangeConfig{
			"A": {Nombre: "A", FeeTaker: 0.001},
			"B": {Nombre: "B", FeeTaker: 0.001},
		},
	}
	ahora := time.Now()
	carteras := NuevaCarteras([]string{"A", "B"}, 100000, 1)
	analizador := NuevoAnalizador(costos)

	oportunidades := analizador.Buscar(map[string]Cotizacion{
		"A": {Exchange: "A", Bid: 69900, Ask: 70000, BidCantidad: 1, AskCantidad: 1, RecibidaEn: ahora},
		"B": {Exchange: "B", Bid: 70300, Ask: 70400, BidCantidad: 1, AskCantidad: 1, RecibidaEn: ahora},
	}, carteras, ahora)

	if len(oportunidades) == 0 {
		t.Fatal("se esperaba una oportunidad")
	}
	if !oportunidades[0].Ejecutable {
		t.Fatalf("se esperaba ejecutable, razon=%s utilidad=%f", oportunidades[0].Razon, oportunidades[0].UtilidadUSD)
	}
}

func TestAnalizadorMarcaOrdenParcialPorLiquidez(t *testing.T) {
	costos := MapaCostos{
		MaxOperacionBTC:       1,
		MinUtilidadUSD:        1,
		MinDiferencialNetoBps: 0.1,
		DeslizamientoBps:      0,
		LatenciaRiesgoBps:     0,
		RetiroAmortizadoBps:   0,
		StaleMs:               1000,
		Exchanges: map[string]ExchangeConfig{
			"A": {Nombre: "A", FeeTaker: 0.0001},
			"B": {Nombre: "B", FeeTaker: 0.0001},
		},
	}
	ahora := time.Now()
	carteras := NuevaCarteras([]string{"A", "B"}, 1000000, 2)
	analizador := NuevoAnalizador(costos)

	oportunidades := analizador.Buscar(map[string]Cotizacion{
		"A": {Exchange: "A", Bid: 69900, Ask: 70000, BidCantidad: 2, AskCantidad: 0.12, RecibidaEn: ahora},
		"B": {Exchange: "B", Bid: 70600, Ask: 70700, BidCantidad: 2, AskCantidad: 2, RecibidaEn: ahora},
	}, carteras, ahora)

	if len(oportunidades) == 0 {
		t.Fatal("se esperaba una oportunidad")
	}
	if !oportunidades[0].Parcial {
		t.Fatalf("se esperaba parcial, cantidad=%f", oportunidades[0].CantidadBTC)
	}
	if oportunidades[0].CantidadBTC != 0.12 {
		t.Fatalf("cantidad incorrecta: %f", oportunidades[0].CantidadBTC)
	}
}
