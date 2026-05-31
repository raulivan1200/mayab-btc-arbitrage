package motor

import "sync"

type Carteras struct {
	mu       sync.RWMutex
	balances map[string]*Balance
	inicial  map[string]Balance
}

func NuevaCarteras(exchanges []string, usdInicial float64, btcInicial float64) *Carteras {
	if len(exchanges) == 0 {
		return &Carteras{
			balances: map[string]*Balance{},
			inicial:  map[string]Balance{},
		}
	}

	balances := make(map[string]*Balance, len(exchanges))
	inicial := make(map[string]Balance, len(exchanges))
	usdPorExchange := usdInicial / float64(len(exchanges))
	btcPorExchange := btcInicial / float64(len(exchanges))
	for _, exchange := range exchanges {
		balance := Balance{
			Exchange: exchange,
			USD:      usdPorExchange,
			BTC:      btcPorExchange,
		}
		copia := balance
		balances[exchange] = &copia
		inicial[exchange] = balance
	}
	return &Carteras{balances: balances, inicial: inicial}
}

func (c *Carteras) Snapshot() []Balance {
	c.mu.RLock()
	defer c.mu.RUnlock()

	salida := make([]Balance, 0, len(c.balances))
	for _, balance := range c.balances {
		salida = append(salida, *balance)
	}
	return salida
}

func (c *Carteras) Balance(exchange string) Balance {
	c.mu.RLock()
	defer c.mu.RUnlock()

	if balance, ok := c.balances[exchange]; ok {
		return *balance
	}
	return Balance{Exchange: exchange}
}

func (c *Carteras) AplicarOperacion(op Operacion) {
	if op.CantidadBTC <= 0 || op.PrecioCompra <= 0 || op.PrecioVenta <= 0 {
		return
	}

	c.mu.Lock()
	defer c.mu.Unlock()

	compra := c.balances[op.CompraEn]
	venta := c.balances[op.VentaEn]
	if compra == nil || venta == nil {
		return
	}

	costosExtra := op.Costos.TotalUSD - op.Costos.FeeCompraUSD - op.Costos.FeeVentaUSD
	if costosExtra < 0 {
		costosExtra = 0
	}

	costoCompra := op.CantidadBTC*op.PrecioCompra + op.Costos.FeeCompraUSD + costosExtra
	ingresoVenta := op.CantidadBTC*op.PrecioVenta - op.Costos.FeeVentaUSD

	compra.USD -= costoCompra
	compra.BTC += op.CantidadBTC
	venta.USD += ingresoVenta
	venta.BTC -= op.CantidadBTC
}

func (c *Carteras) CapitalInicialUSD(precioBTC float64) float64 {
	c.mu.RLock()
	defer c.mu.RUnlock()

	total := 0.0
	for _, balance := range c.inicial {
		total += balance.USD + balance.BTC*precioBTC
	}
	return total
}

func (c *Carteras) CapitalActualUSD(precioBTC float64) float64 {
	c.mu.RLock()
	defer c.mu.RUnlock()

	total := 0.0
	for _, balance := range c.balances {
		total += balance.USD + balance.BTC*precioBTC
	}
	return total
}
