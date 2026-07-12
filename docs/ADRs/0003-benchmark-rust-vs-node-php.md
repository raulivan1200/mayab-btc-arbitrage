# ADR 3: Comparativa de Rendimiento (Rust vs Node/PHP)

## Contexto
El motor de arbitraje requiere latencias ultrabajas (sub-milisecond) para identificar oportunidades y calcular rutas antes de que el mercado se mueva.

## Decisión
Se eligió **Rust** (con `tokio`) frente a Node.js o PHP.
Node.js:
- Single-threaded event loop que introduce latencia de GC (Garbage Collection).
- Menor determinismo en operaciones CPU-bound (cálculo GA y enrutamiento de 10 exchanges).

PHP:
- Arquitectura request-response (fpm), mala para conexiones WebSocket de larga vida concurrentes.
- Fuerte latencia en procesamiento intensivo de memoria compartida (simulaciones, libro de órdenes).

Rust:
- Sin GC, pausas cero.
- Cero costo de abstracción (e.g. `rust_decimal`).
- Multihilo verdadero, escalando con los cores para evaluar múltiples pares simultáneamente.

## Consecuencias
- Curva de aprendizaje más alta.
- Tiempos de compilación mayores.
- Tiempo de ejecución de `Analizador::buscar_oportunidades` en el orden de los microsegundos.
- Consumo de RAM estable y predecible (típicamente < 50MB) frente a varios GB en Node.js.
