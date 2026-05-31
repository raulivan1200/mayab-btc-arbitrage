package motor

import "testing"

func TestNuevaCarterasDistribuyeCapitalInicial(t *testing.T) {
	carteras := NuevaCarteras([]string{"A", "B"}, 1000, 2)

	balanceA := carteras.Balance("A")
	if balanceA.USD != 500 {
		t.Fatalf("USD inicial incorrecto: %f", balanceA.USD)
	}
	if balanceA.BTC != 1 {
		t.Fatalf("BTC inicial incorrecto: %f", balanceA.BTC)
	}

	capital := carteras.CapitalInicialUSD(100)
	if capital != 1200 {
		t.Fatalf("capital inicial incorrecto: %f", capital)
	}
}

func TestAplicarOperacionReflejaCostosTotales(t *testing.T) {
	carteras := NuevaCarteras([]string{"Compra", "Venta"}, 100000, 2)
	costos := CostosOperacion{
		FeeCompraUSD:      1,
		FeeVentaUSD:       2,
		DeslizamientoUSD:  3,
		RetiroAmortUSD:    4,
		LatenciaRiesgoUSD: 5,
		TotalUSD:          15,
	}
	operacion := Operacion{
		CompraEn:     "Compra",
		VentaEn:      "Venta",
		CantidadBTC:  0.5,
		PrecioCompra: 100,
		PrecioVenta:  120,
		Costos:       costos,
		UtilidadUSD:  -5,
	}

	carteras.AplicarOperacion(operacion)

	cambioCapital := carteras.CapitalActualUSD(110) - carteras.CapitalInicialUSD(110)
	if cambioCapital != operacion.UtilidadUSD {
		t.Fatalf("cambio de capital incorrecto: %f", cambioCapital)
	}
}
