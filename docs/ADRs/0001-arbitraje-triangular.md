# ADR 1: Implementación de Arbitraje Triangular

## Contexto
El sistema soportaba arbitraje simple (comprar en un exchange, vender en otro). El mercado de criptomonedas también ofrece arbitraje triangular aprovechando ineficiencias entre 3 pares (e.g., BTC/USD, ETH/BTC, ETH/USD).

## Decisión
Se decidió implementar soporte nativo para arbitraje triangular identificando ciclos de 3 activos. Se modificó el `Motor` para calcular ciclos y estimar rentabilidad con un costo compuesto de spread y fees.

## Consecuencias
- Mayor complejidad en el motor y cálculo de rentabilidad.
- Aumenta las oportunidades detectables, permitiendo un PnL mayor.
