package motor

import "sync"

type Carteras struct {
	mu       sync.RWMutex
	balances map[string]*Balance
	inicial  map[string]Balance
}

func NuevaCarteras(exchanges []string, usdInicial float64, btcInicial float64) *Carteras {
	balances := make(map[string]*Balance, len(exchanges))
	inicial := make(map[string]Balance, len(exchanges))
	for _, exchange := range exchanges {
		balance := Balance{
			Exchange: exchange,
			USD:      usdInicial / float64(len(exchanges)),
			BTC:      btcInicial,
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
	c.mu.Lock()
	defer c.mu.Unlock()

	compra := c.balances[op.CompraEn]
	venta := c.balances[op.VentaEn]
	if compra == nil || venta == nil {
		return
	}

	costoCompra := op.CantidadBTC*op.PrecioCompra + op.Costos.FeeCompraUSD
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
