# ADR 0002: Algoritmo Genético Single-Objetivo (no NSGA-II)

## Contexto
El motor de arbitraje requiere optimizar los parámetros de trading según condiciones de mercado y aprender de la historia reciente.

## Decisión
Se implementó un algoritmo genético **single-objetivo** en Rust (`src/ga.rs`).
Optimiza una única función de fitness escalar: utilidad neta esperada ajustada por riesgo y penalizaciones.
El campo `frontera_pareto` en los contratos públicos (`types.rs`) permanece vacío (`vec![]`) documentado como no implementado.

## Consecuencias

### Positivas
- Honesto: no promete multi-objetivo ni NSGA-II real.
- La ejecución es determinística y veloz (< 500 ms).
- Mantiene la premisa de "demo segura" sin dependencias pesadas de ML.

### Negativas
- No explora trade-offs multi-objetivo reales (solo un fitness escalar).
- Puede sobreajustarse a ventanas cortas si no se gestiona bien la población.