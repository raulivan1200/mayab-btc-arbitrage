package motor

import (
	"fmt"
	"math"
	"sort"
	"time"
)

type Analizador struct {
	costos MapaCostos
}

func NuevoAnalizador(costos MapaCostos) *Analizador {
	return &Analizador{costos: costos}
}

func (a *Analizador) Buscar(cotizaciones map[string]Cotizacion, carteras *Carteras, ahora time.Time) []Oportunidad {
	oportunidades := make([]Oportunidad, 0, len(cotizaciones)*len(cotizaciones))

	for nombreCompra, compra := range cotizaciones {
		if !cotizacionValida(compra, ahora, a.costos.StaleMs) {
			continue
		}
		for nombreVenta, venta := range cotizaciones {
			if nombreCompra == nombreVenta || !cotizacionValida(venta, ahora, a.costos.StaleMs) {
				continue
			}

			oportunidad := a.calcularOportunidad(compra, venta, carteras, ahora)
			if oportunidad.DiferencialBrutoUSD <= 0 {
				continue
			}
			oportunidades = append(oportunidades, oportunidad)
		}
	}

	sort.Slice(oportunidades, func(i, j int) bool {
		if oportunidades[i].Ejecutable != oportunidades[j].Ejecutable {
			return oportunidades[i].Ejecutable
		}
		return oportunidades[i].UtilidadUSD > oportunidades[j].UtilidadUSD
	})

	return oportunidades
}

func (a *Analizador) calcularOportunidad(compra Cotizacion, venta Cotizacion, carteras *Carteras, ahora time.Time) Oportunidad {
	ask := compra.Ask
	bid := venta.Bid
	diferencialBruto := bid - ask
	precioMedio := (ask + bid) / 2
	latenciaMax := max(compra.LatenciaMs, venta.LatenciaMs)
	balanceCompra := carteras.Balance(compra.Exchange)
	balanceVenta := carteras.Balance(venta.Exchange)
	feeCompra := a.configExchange(compra.Exchange).FeeTaker

	liquidezCompra := cantidadSegura(compra.AskCantidad)
	liquidezVenta := cantidadSegura(venta.BidCantidad)
	porUSD := balanceCompra.USD / (ask * (1 + feeCompra))
	porBTC := balanceVenta.BTC
	cantidadDeseada := a.costos.MaxOperacionBTC
	cantidad := minPositiva(cantidadDeseada, liquidezCompra, liquidezVenta, porUSD, porBTC)

	costos := a.calcularCostos(cantidad, ask, bid, latenciaMax, compra.Exchange, venta.Exchange)
	utilidad := diferencialBruto*cantidad - costos.TotalUSD
	diferencialNetoUnidad := 0.0
	if cantidad > 0 {
		diferencialNetoUnidad = utilidad / cantidad
	}
	diferencialBrutoBps := bps(diferencialBruto, precioMedio)
	diferencialNetoBps := bps(diferencialNetoUnidad, precioMedio)

	razon := "rentable"
	ejecutable := true
	if cantidad <= 0 {
		ejecutable = false
		razon = "sin liquidez o balance suficiente"
	} else if utilidad < a.costos.MinUtilidadUSD {
		ejecutable = false
		razon = "utilidad menor al mínimo configurado"
	} else if diferencialNetoBps < a.costos.MinDiferencialNetoBps {
		ejecutable = false
		razon = "diferencial neto bajo después de costos"
	} else if latenciaMax > a.costos.StaleMs {
		ejecutable = false
		razon = "cotización antigua"
	}

	return Oportunidad{
		ID:                  fmt.Sprintf("%s-%s-%d", compra.Exchange, venta.Exchange, ahora.UnixNano()),
		CompraEn:            compra.Exchange,
		VentaEn:             venta.Exchange,
		Ask:                 ask,
		Bid:                 bid,
		DiferencialBrutoUSD: diferencialBruto,
		DiferencialBrutoBps: diferencialBrutoBps,
		DiferencialNetoUSD:  diferencialNetoUnidad,
		DiferencialNetoBps:  diferencialNetoBps,
		CantidadBTC:         cantidad,
		UtilidadUSD:         utilidad,
		Costos:              costos,
		LatenciaMaxMs:       latenciaMax,
		DetectadaEn:         ahora,
		Razon:               razon,
		Ejecutable:          ejecutable,
		Parcial:             cantidad > 0 && cantidad < cantidadDeseada*0.999,
	}
}

func (a *Analizador) calcularCostos(cantidad float64, ask float64, bid float64, latenciaMs int64, compraEn string, ventaEn string) CostosOperacion {
	if cantidad <= 0 {
		return CostosOperacion{}
	}

	feeCompraUSD := cantidad * ask * a.configExchange(compraEn).FeeTaker
	feeVentaUSD := cantidad * bid * a.configExchange(ventaEn).FeeTaker
	precioMedio := (ask + bid) / 2
	deslizamientoUSD := cantidad * precioMedio * a.costos.DeslizamientoBps / 10000
	volumenRebalance := max(a.costos.MaxOperacionBTC*20, 1)
	retiroFijoBTC := a.configExchange(compraEn).RetiroBTC + a.configExchange(ventaEn).RetiroBTC
	retiroAmortUSD := cantidad*precioMedio*a.costos.RetiroAmortizadoBps/10000 + precioMedio*retiroFijoBTC*cantidad/volumenRebalance
	latenciaRiesgoUSD := cantidad * precioMedio * a.costos.LatenciaRiesgoBps * float64(max(latenciaMs, 1)) / 10000 / 100
	total := feeCompraUSD + feeVentaUSD + deslizamientoUSD + retiroAmortUSD + latenciaRiesgoUSD

	return CostosOperacion{
		FeeCompraUSD:      feeCompraUSD,
		FeeVentaUSD:       feeVentaUSD,
		DeslizamientoUSD:  deslizamientoUSD,
		RetiroAmortUSD:    retiroAmortUSD,
		LatenciaRiesgoUSD: latenciaRiesgoUSD,
		TotalUSD:          total,
	}
}

func (a *Analizador) configExchange(nombre string) ExchangeConfig {
	if config, ok := a.costos.Exchanges[nombre]; ok {
		return config
	}
	return ExchangeConfig{Nombre: nombre, FeeTaker: 0.0015, RetiroBTC: 0.00015, Confiabilidad: 0.90}
}

func cotizacionValida(c Cotizacion, ahora time.Time, staleMs int64) bool {
	if c.Exchange == "" || c.Bid <= 0 || c.Ask <= 0 || c.Bid >= c.Ask {
		return false
	}
	edad := ahora.Sub(c.RecibidaEn).Milliseconds()
	return edad <= staleMs
}

func cantidadSegura(cantidad float64) float64 {
	if cantidad <= 0 || math.IsNaN(cantidad) || math.IsInf(cantidad, 0) {
		return 0.025
	}
	return cantidad
}

func bps(valor float64, base float64) float64 {
	if base <= 0 {
		return 0
	}
	return valor / base * 10000
}

func minPositiva(valores ...float64) float64 {
	minimo := math.MaxFloat64
	for _, valor := range valores {
		if valor <= 0 || math.IsNaN(valor) || math.IsInf(valor, 0) {
			return 0
		}
		if valor < minimo {
			minimo = valor
		}
	}
	if minimo == math.MaxFloat64 {
		return 0
	}
	return minimo
}
