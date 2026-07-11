# Guía de Pruebas (Demo Script) para Jurado

Este documento detalla el paso a paso para probar todas las funcionalidades principales, mecanismos de robustez y algoritmos del motor.

## Recorrido opcional de 2 minutos

- **Acción:** En el encabezado del dashboard, pulse **Recorrido de 2 min**.
- **Expectativa:** La guía cambia de pestaña y resalta, en orden, lectura ejecutiva, feeds, cálculo neto, escenarios adversos, validación multisemilla y campeón GA.
- **Atajo reproducible:** En "Demo controlada", use **Reiniciar corrida de jurado** antes de **Preparar recorrido completo**. El reset conserva feeds públicos y configuración, pero limpia balances, PnL, riesgo y estado GA.

## 1. Validación de Readiness Inicial
- **Acción:** Al cargar el dashboard (http://127.0.0.1:8080), observe la sección central **"Readiness"** (Modo Jurado).
- **Expectativa:** 5 tarjetas deben mostrar "Ok" validando la parametrización, robustez, soporte multi-wallet, métricas de latencia y documentación.

## 2. Inyectar Oportunidad (Demo Rentable)
- **Acción:** Pulse **"Preparar demo auditada"** en el resumen o desplácese a "Demo controlada" y pulse **"Preparar recorrido completo"**. La acción explícita reinicia primero la corrida simulada para que visitas previas no acumulen PnL.
- **Expectativa:** 
  1. El PnL debe incrementar.
  2. En "Ejecución -> Operaciones" debe aparecer una transacción rentable en verde.
  3. El panel "GA Lab" debe reportar que la generación avanzó y ajustó parámetros.

## 3. Revisar el "Decision Inspector"
- **Acción:** En la tabla "Oportunidades", haga click sobre alguna fila.
- **Expectativa:** El recuadro inferior "Forense" mostrará los desglose de costos (Slippage, Fees, Riesgo latencia) y un Badge colorido. Podrá ver el código `ACEPTADA` o `RECHAZADA_` junto con el razonamiento.

## 4. Escenario: Circuit Breaker
- **Acción:** En "Demo controlada", presione **"Circuit breaker"**.
- **Expectativa:** 
  1. Aparecerá un banner superior rojo alertando la detención de las ejecuciones.
  2. El "Modo de Operación" en la esquina superior izquierda se tornará Ámbar/Rojo.
  3. Las siguientes inyecciones de **"Repetir escenario rentable"** serán rechazadas con `RECHAZADA_CIRCUIT_BREAKER`.

## 4.1 Prueba de caos completa
- **Acción:** Presione **"Prueba de caos completa"**.
- **Expectativa:** El motor encadena fill parcial, baja liquidez, fallo de segunda pierna con unwind, circuit breaker, rebalanceo y recuperación. El resultado debe mostrar `8/8 checks`, exposición residual `0 BTC` y circuit breaker restaurado.
- **API equivalente:** `curl -X POST http://127.0.0.1:8080/api/demo/caos`.

## 5. Escenario: Rebalanceo de Carteras
- **Acción:** Presione **"Forzar rebalanceo"**.
- **Expectativa:** 
  1. En la tabla "Wallets -> Rebalanceos" aparecerá un nuevo registro.
  2. En el panel "Carteras", el renglón de "Total Costos Reb." aumentará en color rojo.

## 6. Generación y Exportación de Evidencia
- **Acción:** En la sección "Qué está pasando ahora", haga click en **"Exportar CSV"**.
- **Expectativa:** Se descargará un CSV completo que contiene en la parte superior el volcado de la configuración usada, y abajo la sábana completa de transacciones y decisiones algorítmicas que explican el comportamiento del sistema.
