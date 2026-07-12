# ADR 2: Algoritmo Genético Single-Objetivo (no NSGA-II)

## Contexto
El algoritmo genético utiliza una única función de fitness escalar (utilidad neta esperada ajustada por riesgo). No se implementó NSGA-II ni frontera de Pareto multi-objetivo real.

## Decisión
Mantener un GA single-objetivo honesto: fitness = utilidad neta esperada - penalización riesgo. El campo `frontera_pareto` en los contratos permanece vacío (`vec![]`) documentado como no implementado.

## Consecuencias
- Evita over-engineering y promesas falsas de multi-objetivo.
- La frontera de Pareto (`frontera_pareto: vec![]`) está documentada como no implementada en los contratos públicos (`types.rs`).
- La convergencia es más rápida y predecible para el objetivo único de utilidad neta.