package mercado

import (
	"math"
	"strconv"
	"strings"
	"time"
)

func decimalObligatorio(valor string) (float64, bool) {
	numero, ok := decimal(valor)
	if !ok || numero <= 0 {
		return 0, false
	}
	return numero, true
}

func decimalOpcional(valor string) float64 {
	numero, ok := decimal(valor)
	if !ok || numero < 0 {
		return 0
	}
	return numero
}

func decimal(valor string) (float64, bool) {
	valor = strings.TrimSpace(valor)
	if valor == "" {
		return 0, false
	}
	numero, err := strconv.ParseFloat(valor, 64)
	if err != nil || math.IsNaN(numero) || math.IsInf(numero, 0) {
		return 0, false
	}
	return numero, true
}

func marcaTiempoUnixMs(valor string) int64 {
	valor = strings.TrimSpace(valor)
	if valor == "" {
		return 0
	}
	marca, err := strconv.ParseInt(valor, 10, 64)
	if err != nil || marca < 0 {
		return 0
	}
	return marca
}

func marcaTiempoRFC3339Nano(valor string) int64 {
	valor = strings.TrimSpace(valor)
	if valor == "" {
		return 0
	}
	marca, err := time.Parse(time.RFC3339Nano, valor)
	if err != nil {
		return 0
	}
	return marca.UnixMilli()
}
