# ADR 0001: Arquitectura del Sistema

## Contexto

Se requiere un sistema para demostrar el funcionamiento de un motor de arbitraje de criptomonedas (Bitcoin) en tiempo real, de manera segura y sin acceso real a llaves privadas o fondos. El sistema debe poder ser evaluado por un jurado bajo diversas condiciones de mercado simuladas.

## Decisión

Se optó por una arquitectura monolítica en Rust compuesta por:
1. **Servidor Axum:** Para servir la API HTTP y manejar conexiones WebSocket.
2. **Motor de Arbitraje Simulado:** Un estado compartido en memoria (`Arc<Motor>`) que evalúa las oportunidades basándose en feeds de precios públicos y parámetros dinámicos.
3. **Frontend SPA (Vanilla JS + CSS):** Un dashboard estático para la interacción y visualización de datos en tiempo real.

## Consecuencias

### Positivas
- Alta eficiencia y latencia baja gracias a Rust y Tokio.
- Fácil despliegue (un solo binario, cero dependencias externas de bases de datos).
- Totalmente seguro, ya que las operaciones son 100% simuladas y solo ocurren en la memoria local del servidor.

### Negativas
- El estado se pierde si el servidor se reinicia, lo cual es aceptable para una demo pero no para un sistema de producción real.
