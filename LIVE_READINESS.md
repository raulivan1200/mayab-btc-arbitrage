# Fase 8 — Live readiness

Este documento es una lista de gates de seguridad y operación. No certifica al sistema como listo para producción institucional ni implica que el dashboard público opere fondos.

## Estado público del challenge

- [x] El proyecto público permanece limitado a **S0 Simulación** y **S1 Replay real**.
- [x] No usa llaves privadas de trading, no envía órdenes reales y no custodia fondos.
- [x] Los endpoints mutables solo alteran estado simulado en memoria.
- [ ] Cualquier evidencia de testnet se presenta por separado y se etiqueta como sandbox con capital falso.
- [ ] Ninguna evidencia de testnet se muestra como prueba de operación con fondos desde el dashboard público.

## Gates por etapa

Avanzar de etapa requiere completar todos los gates de salida de la etapa actual. Un gate fallido o sin evidencia impide avanzar.

| Etapa | Ejecución | Capital | Gate de salida | Estado del challenge público |
|---|---|---:|---|---|
| S0 Simulación | Sintética | $0 | Invariantes y pruebas de caos pasan | En alcance |
| S1 Replay real | Offline | $0 | Auditor y holdout pasan | En alcance |
| S2 Shadow | Datos live, sin órdenes | $0 | Latencia y decisiones permanecen estables | Fuera de alcance público |
| S3 Testnet | Órdenes sandbox | Falso | Ciclo completo y reconciliación pasan | Evidencia adicional separada |
| S4 Production read-only | Llave View | $0 | Permisos, secretos e IP están verificados | Fuera de alcance |
| S5 Canary | Trade-only | Mínimo | Límites y kill switch están probados | Fuera de alcance |
| S6 Live controlado | Trade-only | Acotado | Aprobación humana y monitoreo están activos | Fuera de alcance |

## Gate S0 — Simulación

- [ ] Los invariantes de balances, exposición y PnL pasan de forma determinista.
- [ ] Las pruebas de caos cubren rechazo, movimiento de mercado, fill parcial, unwind, circuit breaker y recuperación.
- [ ] El escenario `mercado_rentable` mantiene viva la demo sin depender de oportunidades reales.
- [ ] Los eventos y resultados sintéticos se etiquetan inequívocamente como simulados.

## Gate S1 — Replay real

- [ ] El replay usa datos de mercado reales capturados, sin conexión a endpoints privados de trading.
- [ ] El auditor puede reconstruir decisiones, costos, balances y resultados desde la evidencia persistida.
- [ ] El holdout está separado del conjunto usado para ajustar o evolucionar la estrategia.
- [ ] Los resultados del holdout cumplen los umbrales definidos antes de ejecutar la evaluación.
- [ ] La corrida es reproducible y conserva la procedencia e integridad de los datos.

## Gate S2 — Shadow

- [ ] Consume datos live únicamente con acceso público o read-only.
- [ ] No crea, modifica ni cancela órdenes.
- [ ] La latencia, staleness, tasa de decisiones y exposición hipotética permanecen dentro de límites definidos.
- [ ] Las decisiones son estables durante la ventana de observación acordada.
- [ ] Un fallo revierte el sistema a replay o simulación.

## Gate S3 — Testnet

- [ ] Las credenciales solo tienen permisos sandbox y no funcionan en producción.
- [ ] El ciclo crear, consultar, cancelar y reconciliar órdenes se completa correctamente.
- [ ] Fills parciales, rechazos, timeouts y reinicios se reconcilian contra el estado del exchange.
- [ ] Toda UI, log y evidencia identifica el entorno como testnet y el capital como falso.
- [ ] La evidencia se mantiene separada del dashboard público del challenge.

## Gate S4 — Production read-only

- [ ] La llave tiene exclusivamente permiso **View**.
- [ ] Trading, transferencias y retiros están deshabilitados en el exchange.
- [ ] Los secretos se almacenan fuera del repositorio, logs, frontend y artefactos de build.
- [ ] La rotación y revocación de secretos están probadas.
- [ ] La allowlist de IP está configurada y verificada.
- [ ] Los accesos quedan auditados.

## Gates iniciales S5 — Canary

Todos estos límites deben implementarse como controles duros y probarse antes de usar capital real:

- [ ] Máximo de una orden abierta.
- [ ] Notional máximo equivalente a **5–10 USD**.
- [ ] Sin market orders inicialmente.
- [ ] Pérdida diaria máxima definida y aplicada automáticamente.
- [ ] Exposición BTC máxima definida y aplicada automáticamente.
- [ ] Máximo de un unwind por incidente.
- [ ] Máximo de órdenes por minuto.
- [ ] Staleness máxima definida; los datos más antiguos bloquean nuevas órdenes.
- [ ] Latencia máxima definida; excederla bloquea nuevas órdenes.
- [ ] Kill switch manual probado.
- [ ] Kill switch automático probado.
- [ ] Cero permisos y cero operaciones de transferencias o retiros.
- [ ] Rollback a shadow mode probado.
- [ ] Reconciliación confirma que no quedan órdenes ni exposición desconocidas al detenerse.

## Gate S6 — Live controlado

- [ ] El capital y la exposición tienen límites explícitos y acotados.
- [ ] Cada aumento de límites requiere aprobación humana registrada.
- [ ] El monitoreo cubre órdenes, fills, exposición, PnL, latencia, staleness y reconciliación.
- [ ] Hay alertas y responsable de respuesta definidos.
- [ ] Los procedimientos de pausa, kill switch, rollback y recuperación están ensayados.
- [ ] No se habilitan transferencias ni retiros como parte del motor de ejecución.

## Regla de comunicación

- [x] La aplicación pública se describe como simulación/replay, no como sistema que opera fondos.
- [ ] Testnet, si se implementa, se presenta únicamente como evidencia técnica adicional.
- [ ] Ninguna etapa futura se declara completada sin evidencia verificable de todos sus gates.
- [ ] “Live”, cuando describa datos de mercado, se distingue explícitamente de “trading live”.
