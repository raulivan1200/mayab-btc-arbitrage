let ultimoEstado = null;
let tieneCambios = false;
let oportunidadSeleccionadaId = null;
const gaHistorial = [];
const INTERVALO_AUTO_GA_MS = 6500;
let gaAutoEnCurso = false;
const CLAVE_LIDER_GA = "mayabGaAutoLeader";
const ID_PESTANA = `tab-${Date.now()}-${Math.random().toString(16).slice(2)}`;
const DEBUG_ACTIVO =
  new URLSearchParams(location.search).has("debug") ||
  localStorage.getItem("mayabDebug") === "1";
const INTERVALO_CANVAS_MS = 1000 / 30;
let ultimoFrameCanvas = 0;
let ultimoPreflightMs = 0;
let preflightCache = null;
let preflightEnCurso = false;

const metricasPrevias = {
  pnl: 0,
  retorno: 0,
  eventos: 0,
  latencia: 0,
  sharpe: 0,
  winRate: 0,
  maxDrawdown: 0,
  operacionesTotales: 0,
  operacionesFallidas: 0,
  rebalanceosTotales: 0,
};

const opsNotificadas = new Set();
const metricasDebug = DEBUG_ACTIVO ? crearDebugMetrics() : null;

const $ = (id) => document.getElementById(id);
const dinero = new Intl.NumberFormat("es-MX", {
  style: "currency",
  currency: "USD",
  maximumFractionDigits: 2,
});
const numero = new Intl.NumberFormat("es-MX", { maximumFractionDigits: 2 });
const btc = new Intl.NumberFormat("es-MX", { maximumFractionDigits: 6 });

function mostrarFeedback(el, mensaje, ok = true) {
  if (!el) return;
  el.textContent = mensaje;
  el.style.color = ok ? "var(--verde)" : "var(--rojo)";
}

async function mensajeErrorApi(res, fallback) {
  try {
    const body = await res.clone().json();
    return body?.error?.message || body?.message || fallback;
  } catch (_) {
    return fallback;
  }
}

function marcarCambio(el) {
  if (!el) return;
  el.classList.remove("pulse-verde", "ga-flash");
  void el.offsetWidth;
  el.classList.add("pulse-verde", "ga-flash");
}

function crearDebugMetrics() {
  return {
    inicio: performance.now(),
    wsMensajes: 0,
    wsBytes: 0,
    renders: 0,
    framesCanvas: 0,
    longTasks: 0,
    medidas: new Map(),
  };
}

function debugNow() {
  return DEBUG_ACTIVO ? performance.now() : 0;
}

function debugMeasure(nombre, inicio) {
  if (!DEBUG_ACTIVO || !inicio) return;
  const duracion = performance.now() - inicio;
  const actual = metricasDebug.medidas.get(nombre) || { n: 0, total: 0, max: 0 };
  actual.n += 1;
  actual.total += duracion;
  actual.max = Math.max(actual.max, duracion);
  metricasDebug.medidas.set(nombre, actual);
}

function debugLog(...args) {
  if (DEBUG_ACTIVO) console.debug("[mayab-debug]", ...args.map(debugSerialize));
}

function debugWarn(...args) {
  if (DEBUG_ACTIVO) console.warn("[mayab-debug]", ...args.map(debugSerialize));
}

function debugError(...args) {
  if (DEBUG_ACTIVO) console.error("[mayab-debug]", ...args.map(debugSerialize));
}

function debugSerialize(valor) {
  if (valor instanceof Error) return `${valor.name}: ${valor.message}`;
  if (typeof valor === "object" && valor !== null) {
    try {
      return JSON.stringify(valor);
    } catch (_) {
      return String(valor);
    }
  }
  return valor;
}

function iniciarDebug() {
  if (!DEBUG_ACTIVO) return;
  window.mayabDebugMetrics = metricasDebug;
  document.documentElement.dataset.mayabDebug = "1";
  debugLog("debug activo", { tab: ID_PESTANA });
  if ("PerformanceObserver" in window) {
    try {
      const observer = new PerformanceObserver((list) => {
        metricasDebug.longTasks += list.getEntries().length;
      });
      observer.observe({ type: "longtask", buffered: true });
    } catch (err) {
      debugWarn("longtask observer no disponible", err);
    }
  }
  setInterval(() => {
    const medidas = Object.fromEntries(
      [...metricasDebug.medidas.entries()].map(([nombre, m]) => [
        nombre,
        {
          n: m.n,
          avgMs: Number((m.total / Math.max(m.n, 1)).toFixed(2)),
          maxMs: Number(m.max.toFixed(2)),
        },
      ]),
    );
    debugLog("perf", {
      uptimeSeg: Number(((performance.now() - metricasDebug.inicio) / 1000).toFixed(1)),
      wsMensajes: metricasDebug.wsMensajes,
      wsKb: Number((metricasDebug.wsBytes / 1024).toFixed(1)),
      renders: metricasDebug.renders,
      framesCanvas: metricasDebug.framesCanvas,
      longTasks: metricasDebug.longTasks,
      medidas,
    });
  }, 5000);
}

// Inicializar configuración y tema
iniciarDebug();
iniciarTema();
conectar();
cargarConfigGa();
iniciarBacktest();
iniciarPresets();
iniciarDemo();
iniciarAutoGa();
setInterval(verificarConexion, 900);

function loopAnimacion(timestamp) {
  if (tieneCambios && ultimoEstado) {
    const inicio = debugNow();
    renderizar(ultimoEstado);
    if (DEBUG_ACTIVO) metricasDebug.renders += 1;
    debugMeasure("render", inicio);
    tieneCambios = false;
  }
  if (ultimoEstado && timestamp - ultimoFrameCanvas >= INTERVALO_CANVAS_MS) {
    const inicio = debugNow();
    dibujarMapa(ultimoEstado);
    dibujarGa(ultimoEstado.genetico);
    ultimoFrameCanvas = timestamp;
    if (DEBUG_ACTIVO) metricasDebug.framesCanvas += 1;
    debugMeasure("canvas", inicio);
  }
  requestAnimationFrame(loopAnimacion);
}
requestAnimationFrame(loopAnimacion);

async function conectar() {
  const protocolo = location.protocol === "https:" ? "wss" : "ws";
  const socket = new WebSocket(`${protocolo}://${location.host}/tiempo-real`);
  cambiarSocket("conectando");

  socket.addEventListener("open", () => cambiarSocket("en vivo", true));
  
  socket.addEventListener("message", (evento) => {
    try {
      const inicio = debugNow();
      const datos = JSON.parse(evento.data);
      if (DEBUG_ACTIVO) {
        metricasDebug.wsMensajes += 1;
        metricasDebug.wsBytes += evento.data.length || 0;
      }
      debugMeasure("ws-parse", inicio);
      ultimoEstado = datos;
      estado.ultimoMensaje = Date.now();
      cambiarSocket("en vivo", true);
      tieneCambios = true;
      detectarNotificaciones(datos);
    } catch (err) {
      debugError("Error parseando WebSocket:", err);
    }
  });

  socket.addEventListener("close", () => {
    cambiarSocket("reconectando");
    debugWarn("websocket cerrado; reconectando");
    setTimeout(conectar, 1200);
  });
  
  socket.addEventListener("error", (err) => {
    cambiarSocket("sin enlace", false);
    debugError("websocket error", err);
  });
}

const estado = {
  ultimoMensaje: 0,
};

function verificarConexion() {
  if (!estado.ultimoMensaje) return;
  const viejo = Date.now() - estado.ultimoMensaje > 2400;
  if (viejo) cambiarSocket("sin datos", false);
}

function cambiarSocket(texto, ok) {
  const el = $("estadoSocket");
  if (!el) return;
  el.classList.toggle("ok", ok === true);
  el.classList.toggle("error", ok === false);
  el.lastChild.nodeValue = ` ${texto}`;
}

function iniciarTema() {
  const toggle = $("themeToggle");
  if (!toggle) return;
  
  const temaGuardado = localStorage.getItem("tema") || "dark";
  document.documentElement.setAttribute("data-theme", temaGuardado);
  actualizarIconosTema(temaGuardado);

  toggle.addEventListener("click", () => {
    const temaActual = document.documentElement.getAttribute("data-theme");
    const nuevoTema = temaActual === "dark" ? "light" : "dark";
    document.documentElement.setAttribute("data-theme", nuevoTema);
    localStorage.setItem("tema", nuevoTema);
    actualizarIconosTema(nuevoTema);
    tieneCambios = true; // Forzar redibujado de canvases
  });
}

function actualizarIconosTema(tema) {
  const sun = document.querySelector(".icon-sun");
  const moon = document.querySelector(".icon-moon");
  const metaColor = $("themeMetaColor");
  
  if (tema === "dark") {
    if (sun) sun.style.display = "block";
    if (moon) moon.style.display = "none";
    if (metaColor) metaColor.setAttribute("content", "#0c0e14");
  } else {
    if (sun) sun.style.display = "none";
    if (moon) moon.style.display = "block";
    if (metaColor) metaColor.setAttribute("content", "#f3f0e8");
  }
}

function aplicarAnimacionCambio(el, nuevoValor, viejaClave) {
  const viejoValor = metricasPrevias[viejaClave];
  if (nuevoValor === viejoValor) return;
  
  el.classList.remove("pulse-verde", "pulse-rojo");
  void el.offsetWidth; // trigger reflow
  
  if (nuevoValor > viejoValor) {
    el.classList.add("pulse-verde");
  } else if (nuevoValor < viejoValor) {
    el.classList.add("pulse-rojo");
  }
  metricasPrevias[viejaClave] = nuevoValor;
}

function renderizar(datos) {
  // Métricas principales
  const pnlVal = datos.metricas.utilidadAcumuladaUsd;
  const pnlEl = $("pnl");
  pnlEl.textContent = dinero.format(pnlVal);
  aplicarAnimacionCambio(pnlEl, pnlVal, "pnl");
  actualizarDetallePnl(datos);

  const retornoVal = datos.metricas.retornoBps;
  const retornoEl = $("retorno");
  retornoEl.textContent = `${formato(retornoVal, 2)} bps`;
  aplicarAnimacionCambio(retornoEl, retornoVal, "retorno");

  const eventosVal = datos.metricas.eventosMercado;
  const eventosEl = $("eventos");
  eventosEl.textContent = numero.format(eventosVal);
  aplicarAnimacionCambio(eventosEl, eventosVal, "eventos");

  const latenciaVal = datos.metricas.latenciaPromedioMs;
  const latenciaEl = $("latencia");
  latenciaEl.textContent = `${formato(latenciaVal, 0)} ms`;
  aplicarAnimacionCambio(latenciaEl, latenciaVal, "latencia");

  // Métricas secundarias
  const sharpeVal = datos.metricas.sharpeRatio;
  const sharpeEl = $("sharpe");
  sharpeEl.textContent = formato(sharpeVal, 2);
  aplicarAnimacionCambio(sharpeEl, sharpeVal, "sharpe");

  const winRateVal = datos.metricas.winRate;
  const winRateEl = $("winRate");
  winRateEl.textContent = `${formato(winRateVal * 100, 1)}%`;
  aplicarAnimacionCambio(winRateEl, winRateVal, "winRate");

  const drawdownVal = datos.metricas.maxDrawdownUsd;
  const drawdownEl = $("maxDrawdown");
  drawdownEl.textContent = dinero.format(drawdownVal);
  aplicarAnimacionCambio(drawdownEl, drawdownVal, "maxDrawdown");

  const opsTotalesVal = datos.metricas.operacionesTotales;
  const opsTotalesEl = $("operacionesTotales");
  opsTotalesEl.textContent = numero.format(opsTotalesVal);
  aplicarAnimacionCambio(opsTotalesEl, opsTotalesVal, "operacionesTotales");

  const opsFallidasVal = datos.metricas.operacionesFallidas || 0;
  const opsFallidasEl = $("operacionesFallidas");
  if (opsFallidasEl) {
    opsFallidasEl.textContent = numero.format(opsFallidasVal);
    aplicarAnimacionCambio(opsFallidasEl, opsFallidasVal, "operacionesFallidas");
  }

  const rebalanceosVal = datos.metricas.rebalanceosTotales || 0;
  const rebalanceosEl = $("rebalanceosTotales");
  if (rebalanceosEl) {
    rebalanceosEl.textContent = numero.format(rebalanceosVal);
    aplicarAnimacionCambio(rebalanceosEl, rebalanceosVal, "rebalanceosTotales");
  }

  // Labels generales
  $("riesgo").textContent = datos.metricas.estadoRiesgo;
  $("trabajadores").textContent = `${datos.metricas.trabajadores} trabajadores`;
  $("mejorDiferencial").textContent = mejorDiferencial(datos);

  // Banners y Badges
  const cbBanner = $("circuitBreakerBanner");
  if (cbBanner) {
    cbBanner.hidden = !datos.metricas.circuitBreakerActivo;
  }
  const consBadge = $("modoConservadorBadge");
  if (consBadge) {
    consBadge.hidden = !datos.metricas.modoConservador;
  }
  actualizarModoOperacion(datos);

  // Renderizado optimizado sin innerHTML
  renderMercado(datos);
  renderLatencias(datos);
  renderJudgeReadiness();
  renderBalances(datos);
  renderConfig(datos);
  renderOportunidades(datos);
  renderDetalleOportunidad(datos);
  renderOperaciones(datos);
  renderEventosEjecucion(datos);
  renderRebalanceos(datos);
  renderAuditoriaDecisiones(datos);
  renderGenetico(datos);
  renderResumenLlm(datos);
  actualizarInputsGaUnaVez(datos.genetico);
  renderExchanges(datos);
  dibujarSeries(datos);
  actualizarInputsConfigUnaVez(datos.configuracion);
}

function actualizarModoOperacion(datos) {
  const badge = $("modoOperacionBadge");
  if (!badge) return;
  const usaFallback = (datos.cotizaciones || []).some((c) => c.ultimoMensaje === "rest_fallback");
  const ahora = Date.now();
  const demoActivo = (datos.eventosEjecucion || []).some((e) => {
    const t = Date.parse(e.tiempo || "");
    return String(e.tipo || "").startsWith("demo") && Number.isFinite(t) && ahora - t < 60_000;
  });
  badge.className = "modo-operacion-badge";
  if (demoActivo && usaFallback) {
    badge.textContent = "DEMO + REST";
    badge.classList.add("fallback");
  } else if (demoActivo) {
    badge.textContent = "DEMO + LIVE";
    badge.classList.add("demo");
  } else if (usaFallback) {
    badge.textContent = "REST FALLBACK";
    badge.classList.add("fallback");
  } else {
    badge.textContent = "LIVE WS";
  }
}

function actualizarDetallePnl(datos) {
  const el = $("pnlDetalle");
  if (!el) return;

  const operaciones = datos.metricas.operacionesTotales || 0;
  if (operaciones > 0) {
    el.textContent = `Resultado acumulado de ${numero.format(operaciones)} operaciones simuladas después de costos.`;
    return;
  }

  const oportunidades = datos.oportunidades || [];
  if (oportunidades.length === 0) {
    el.textContent = "Esperando rutas con spread bruto positivo para simular ejecución.";
    return;
  }

  const ejecutables = oportunidades.filter((o) => o.ejecutable);
  if (ejecutables.length > 0) {
    const mejor = ejecutables.sort((a, b) => b.utilidadUsd - a.utilidadUsd)[0];
    el.textContent = `Hay rutas ejecutables; esperando confirmación del motor. Mejor estimada: ${dinero.format(mejor.utilidadUsd)}.`;
    return;
  }

  const mejor = [...oportunidades].sort((a, b) => b.diferencialNetoBps - a.diferencialNetoBps)[0];
  el.textContent = `En cero porque no hay operaciones aceptadas; mejor neto ${formato(mejor.diferencialNetoBps, 2)} bps (${mejor.razon}).`;
}

// Cargar inputs del formulario una vez
let configInicializada = false;
function actualizarInputsConfigUnaVez(c) {
  if (configInicializada) return;
  const maxBtc = $("inputMaxBtc");
  const minBps = $("inputMinBps");
  const deslizamiento = $("inputDeslizamiento");
  const cooldown = $("inputCooldown");
  const minUtilidad = $("inputMinUtilidad");
  const staleMs = $("inputStaleMs");
  const latenciaRiesgo = $("inputLatenciaRiesgo");
  const circuitBreaker = $("inputCircuitBreaker");
  const volatilidad = $("inputVolatilidad");
  const probFallo = $("inputProbFallo");
  const probMovimiento = $("inputProbMovimiento");
  const movimientoBps = $("inputMovimientoBps");
  const rebalanceUmbral = $("inputRebalanceUmbral");
  const rebalanceTransfer = $("inputRebalanceTransfer");
  
  if (maxBtc) maxBtc.value = c.maxOperacionBtc;
  if (minBps) minBps.value = c.minDiferencialNetoBps;
  if (deslizamiento) deslizamiento.value = c.deslizamientoBps;
  if (cooldown) cooldown.value = c.enfriamientoMs;
  if (minUtilidad) minUtilidad.value = c.minUtilidadUsd;
  if (staleMs) staleMs.value = c.staleMs;
  if (latenciaRiesgo) latenciaRiesgo.value = c.latenciaRiesgoBps;
  if (circuitBreaker) circuitBreaker.value = c.circuitBreakerPerdidaUsd;
  if (volatilidad) volatilidad.value = c.volatilidadUmbralBps;
  if (probFallo) probFallo.value = c.probFalloOrden;
  if (probMovimiento) probMovimiento.value = c.probMovimientoBrusco;
  if (movimientoBps) movimientoBps.value = c.movimientoBruscoBps;
  if (rebalanceUmbral) rebalanceUmbral.value = c.rebalanceUmbralPct;
  if (rebalanceTransfer) rebalanceTransfer.value = c.rebalanceMaxTransferPct;

  const btn = $("btnAplicarConfig");
  if (btn) {
    btn.onclick = async () => {
      await aplicarConfig(construirPayloadConfig(), "Configuración guardada");
    };
  }
  configInicializada = true;
}

function construirPayloadConfig() {
  validarInputsConfig();
  return {
    maxOperacionBtc: parseFloat($("inputMaxBtc")?.value),
    minDiferencialNetoBps: parseFloat($("inputMinBps")?.value),
    deslizamientoBps: parseFloat($("inputDeslizamiento")?.value),
    enfriamientoMs: parseInt($("inputCooldown")?.value, 10),
    minUtilidadUsd: parseFloat($("inputMinUtilidad")?.value),
    staleMs: parseInt($("inputStaleMs")?.value, 10),
    latenciaRiesgoBps: parseFloat($("inputLatenciaRiesgo")?.value),
    circuitBreakerPerdidaUsd: parseFloat($("inputCircuitBreaker")?.value),
    volatilidadUmbralBps: parseFloat($("inputVolatilidad")?.value),
    probFalloOrden: parseFloat($("inputProbFallo")?.value),
    probMovimientoBrusco: parseFloat($("inputProbMovimiento")?.value),
    movimientoBruscoBps: parseFloat($("inputMovimientoBps")?.value),
    rebalanceUmbralPct: parseFloat($("inputRebalanceUmbral")?.value),
    rebalanceMaxTransferPct: parseFloat($("inputRebalanceTransfer")?.value),
  };
}

const limitesConfig = {
  inputMaxBtc: [0.01, 10],
  inputMinBps: [0, 100],
  inputDeslizamiento: [0, 50],
  inputCooldown: [0, 10000],
  inputMinUtilidad: [0, 1000],
  inputStaleMs: [100, 30000],
  inputLatenciaRiesgo: [0, 20],
  inputCircuitBreaker: [0, 100000],
  inputVolatilidad: [0, 1000],
  inputProbFallo: [0, 1],
  inputProbMovimiento: [0, 1],
  inputMovimientoBps: [0, 100],
  inputRebalanceUmbral: [0, 100],
  inputRebalanceTransfer: [0, 100],
};

function validarInputsConfig() {
  let ok = true;
  Object.entries(limitesConfig).forEach(([id, [min, max]]) => {
    const input = $(id);
    if (!input) return;
    const value = Number(input.value);
    const valido = Number.isFinite(value) && value >= min && value <= max;
    input.classList.toggle("input-error", !valido);
    input.title = valido ? "" : `Valor permitido: ${min} a ${max}`;
    ok = ok && valido;
  });
  const feedback = $("configFeedback");
  if (!ok) mostrarFeedback(feedback, "Revisa los campos marcados antes de aplicar.", false);
  return ok;
}

async function aplicarConfig(payload, mensajeOk) {
  const feedback = $("configFeedback");
  if (!validarInputsConfig()) return;
  try {
    const res = await fetch("/api/config", {
      method: "POST",
      headers: headersMutacion({ "Content-Type": "application/json" }),
      body: JSON.stringify(payload),
    });
    if (feedback) {
      if (res.ok) {
        mostrarFeedback(feedback, `✓ ${mensajeOk}`, true);
        setTimeout(() => { feedback.textContent = ""; }, 3000);
      } else {
        mostrarFeedback(feedback, `✗ ${await mensajeErrorApi(res, "Error al guardar")}`, false);
      }
    }
  } catch (err) {
    mostrarFeedback(feedback, "✗ Error de red", false);
  }
}

const presets = {
  balanceado: {
    inputMaxBtc: 0.18,
    inputMinBps: 0.65,
    inputDeslizamiento: 0.35,
    inputCooldown: 1400,
    inputMinUtilidad: 1.25,
    inputStaleMs: 4500,
    inputLatenciaRiesgo: 0.08,
    inputCircuitBreaker: 500,
    inputVolatilidad: 50,
    inputProbFallo: 0.015,
    inputProbMovimiento: 0.02,
    inputMovimientoBps: 7,
    inputRebalanceUmbral: 35,
    inputRebalanceTransfer: 35,
  },
  agresivo: {
    inputMaxBtc: 0.35,
    inputMinBps: 0.25,
    inputDeslizamiento: 0.22,
    inputCooldown: 450,
    inputMinUtilidad: 0.5,
    inputStaleMs: 7000,
    inputLatenciaRiesgo: 0.04,
    inputCircuitBreaker: 900,
    inputVolatilidad: 90,
    inputProbFallo: 0.01,
    inputProbMovimiento: 0.015,
    inputMovimientoBps: 5,
    inputRebalanceUmbral: 45,
    inputRebalanceTransfer: 50,
  },
  seguro: {
    inputMaxBtc: 0.08,
    inputMinBps: 1.6,
    inputDeslizamiento: 0.8,
    inputCooldown: 2500,
    inputMinUtilidad: 4,
    inputStaleMs: 2200,
    inputLatenciaRiesgo: 0.22,
    inputCircuitBreaker: 220,
    inputVolatilidad: 28,
    inputProbFallo: 0.015,
    inputProbMovimiento: 0.02,
    inputMovimientoBps: 7,
    inputRebalanceUmbral: 25,
    inputRebalanceTransfer: 25,
  },
  estres: {
    inputMaxBtc: 0.18,
    inputMinBps: 0.9,
    inputDeslizamiento: 1.2,
    inputCooldown: 1100,
    inputMinUtilidad: 1.5,
    inputStaleMs: 2600,
    inputLatenciaRiesgo: 0.35,
    inputCircuitBreaker: 180,
    inputVolatilidad: 22,
    inputProbFallo: 0.14,
    inputProbMovimiento: 0.18,
    inputMovimientoBps: 18,
    inputRebalanceUmbral: 20,
    inputRebalanceTransfer: 25,
  },
};

function iniciarPresets() {
  document.querySelectorAll("[data-preset]").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const nombre = btn.dataset.preset;
      const preset = presets[nombre];
      if (!preset) return;
      Object.entries(preset).forEach(([id, valor]) => {
        const input = $(id);
        if (input) input.value = valor;
      });
      document.querySelectorAll("[data-preset]").forEach((otro) => otro.classList.toggle("activo", otro === btn));
      await aplicarConfig(construirPayloadConfig(), `Preset ${btn.textContent.trim()} aplicado`);
    });
  });
}

function iniciarDemo() {
  document.querySelectorAll("[data-demo]").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const escenario = btn.dataset.demo;
      const feedback = $("demoFeedback");
      const estado = $("demoEstado");
      btn.disabled = true;
      const textoOriginal = btn.textContent;
      btn.textContent = "Ejecutando...";
      try {
        const res = await fetch("/api/demo", {
          method: "POST",
          headers: headersMutacion({ "Content-Type": "application/json" }),
          body: JSON.stringify({ escenario }),
        });
        const ok = res.ok;
        const body = ok ? await res.json() : null;
        if (estado) estado.textContent = ok ? "evento enviado" : "rechazado";
        const detalle = body?.partialFill
          ? `✓ Fill parcial: requested ${btc.format(body.requestedQtyBtc || 0)} BTC, filled ${btc.format(body.filledQtyBtc || 0)} BTC.`
          : body?.operacionesInsertadas
          ? `✓ Demo rentable: ${body.operacionesInsertadas} operaciones insertadas; GA gen ${body.generacionGa}.`
          : "✓ Escenario aplicado; revisa eventos, auditoría y métricas.";
        const error = ok ? "" : await mensajeErrorApi(res, "No se pudo aplicar escenario");
        mostrarFeedback(feedback, ok ? detalle : `✗ ${error}`, ok);
      } catch (e) {
        if (estado) estado.textContent = "error";
        mostrarFeedback(feedback, "✗ Error de red", false);
      } finally {
        btn.disabled = false;
        btn.textContent = textoOriginal;
      }
    });
  });
}

let gaInicializada = false;
async function cargarConfigGa() {
  try {
    const res = await fetch("/api/ga/config");
    if (!res.ok) return;
    const cfg = await res.json();
    actualizarInputsGaUnaVez({
      poblacion: cfg.tamanoPoblacion,
      tasaMutacion: cfg.tasaMutacion,
      tasaCruce: cfg.tasaCruce,
    });
  } catch (e) {
    debugWarn("No se pudo cargar config GA", e);
  }
}

async function actualizarInputsGaUnaVez(g) {
  if (gaInicializada || !g) return;
  const poblacion = $("inputGaPoblacion");
  const mutacion = $("inputGaMutacion");
  const cruce = $("inputGaCruce");
  if (poblacion) poblacion.value = g.poblacion ?? g.tamanoPoblacion ?? 50;
  if (mutacion) mutacion.value = g.tasaMutacion ?? 0.15;
  if (cruce) cruce.value = g.tasaCruce ?? 0.72;

  const feedback = $("gaFeedback");
  const btnAplicar = $("btnAplicarGa");
  if (btnAplicar) {
    btnAplicar.onclick = async () => {
      btnAplicar.disabled = true;
      btnAplicar.textContent = "Aplicando...";
      try {
        const res = await fetch("/api/ga/config", {
          method: "POST",
          headers: headersMutacion({ "Content-Type": "application/json" }),
          body: JSON.stringify({
            tamanoPoblacion: parseInt(poblacion.value),
            tasaMutacion: parseFloat(mutacion.value),
            tasaCruce: parseFloat(cruce.value),
          }),
        });
        const mensaje = res.ok ? "GA actualizado" : await mensajeErrorApi(res, "No se pudo actualizar GA");
        mostrarFeedback(feedback, `${res.ok ? "✓" : "✗"} ${mensaje}`, res.ok);
      } catch (e) {
        mostrarFeedback(feedback, "✗ Error de red", false);
      } finally {
        btnAplicar.disabled = false;
        btnAplicar.textContent = "Aplicar GA";
      }
    };
  }

  const btnEvolucionar = $("btnEvolucionarGa");
  if (btnEvolucionar) {
    btnEvolucionar.onclick = () => evolucionarGa({ manual: true });
  }
  gaInicializada = true;
}

function iniciarAutoGa() {
  setInterval(() => evolucionarGa({ manual: false }), INTERVALO_AUTO_GA_MS);
}

function esLiderAutoGa() {
  const ahora = Date.now();
  try {
    const actual = JSON.parse(localStorage.getItem(CLAVE_LIDER_GA) || "null");
    if (actual && actual.id !== ID_PESTANA && actual.expira > ahora) return false;
    localStorage.setItem(CLAVE_LIDER_GA, JSON.stringify({ id: ID_PESTANA, expira: ahora + INTERVALO_AUTO_GA_MS * 2 }));
    return true;
  } catch (_) {
    return true;
  }
}

function headersMutacion(extra = {}) {
  const headers = { ...extra };
  const token = localStorage.getItem("mayabAdminToken");
  if (token) headers.Authorization = `Bearer ${token}`;
  return headers;
}

async function evolucionarGa({ manual }) {
  if (gaAutoEnCurso) return;
  if (!manual && (!ultimoEstado || document.hidden)) return;
  if (!manual && !esLiderAutoGa()) return;

  const feedback = $("gaFeedback");
  const btnEvolucionar = $("btnEvolucionarGa");
  const genAntes = ultimoEstado?.genetico?.generacion ?? null;
  gaAutoEnCurso = true;

  if (manual && btnEvolucionar) {
      btnEvolucionar.disabled = true;
      btnEvolucionar.textContent = "Evolucionando...";
      mostrarFeedback(feedback, "Evolucionando población...", true);
  }

  try {
    const res = await fetch("/api/ga/evolucionar", {
      method: "POST",
      headers: headersMutacion({ "Content-Type": "application/json" }),
      body: JSON.stringify({ usarReplaySiVacio: true, muestras: 96 }),
    });
    if (!res.ok) {
      if (manual) mostrarFeedback(feedback, `✗ ${await mensajeErrorApi(res, "No se pudo evolucionar")}`, false);
      return;
    }

    const resultado = await res.json();
    const genetico = resultado.ga || await fetch("/api/ga/estado").then((estado) => estado.ok ? estado.json() : null);
    if (!genetico) return;
    renderGenetico({ genetico });
    if (ultimoEstado) ultimoEstado.genetico = genetico;

    const genDespues = genetico.generacion;
    const muestras = genetico.operacionesEvaluadas || 0;
    const fuente = resultado.fuente === "replay_sintetico" ? "replay sintético" : "historial real";
    const prefijo = genAntes === null ? `Gen ${genDespues}` : `Gen ${genAntes} -> ${genDespues}`;
    const detalle = muestras > 0
      ? `${muestras} operaciones evaluadas (${fuente}); campeón vs retador actualizado`
      : "sin operaciones para aprender; población sigue explorando";
    mostrarFeedback(feedback, `${manual ? "✓" : "Auto"} ${prefijo}: ${detalle}`, true);
    marcarCambio($("gaGeneracion"));
    marcarCambio($("gaPesos"));
  } catch (e) {
    if (manual) mostrarFeedback(feedback, "✗ Error de red", false);
  } finally {
    gaAutoEnCurso = false;
    if (manual && btnEvolucionar) {
        btnEvolucionar.disabled = false;
        btnEvolucionar.textContent = "Evolucionar Ahora";
    }
  }
}

function renderMercado(datos) {
  const container = $("exchangeLista");
  if (!container) return;
  
  const diferenciales = datos.cotizaciones.map((c) => c.ask - c.bid).filter((s) => s > 0);
  const maxDiferencial = Math.max(...diferenciales, 1);

  // Limpiar eficientemente
  container.textContent = "";
  
  datos.cotizaciones.forEach((c) => {
    const recibida = Date.parse(c.recibidaEn || "");
    const generado = Date.parse(datos.generadoEn || "");
    const edadMs = Number.isFinite(recibida) && Number.isFinite(generado)
      ? Math.max(0, generado - recibida)
      : 0;
    const fuente = c.ultimoMensaje === "rest_fallback" ? "REST fallback" : "WebSocket";
    const art = document.createElement("article");
    art.className = "exchange";

    const header = document.createElement("header");
    const h3 = document.createElement("h3");
    h3.textContent = c.exchange;
    const latencia = document.createElement("span");
    latencia.className = "latencia-chip";
    latencia.textContent = `${c.latenciaMs || 0} ms`;
    header.appendChild(h3);
    header.appendChild(latencia);

    const precios = document.createElement("div");
    precios.className = "precios";

    const divBid = document.createElement("div");
    divBid.className = "precio bid";
    divBid.innerHTML = `<span>Compra</span><strong>${dinero.format(c.bid)}</strong>`;

    const divAsk = document.createElement("div");
    divAsk.className = "precio ask";
    divAsk.innerHTML = `<span>Venta</span><strong>${dinero.format(c.ask)}</strong>`;

    precios.appendChild(divBid);
    precios.appendChild(divAsk);

    const bar = document.createElement("div");
    bar.className = "barra-diferencial";
    bar.setAttribute("aria-label", "Diferencial");
    const fill = document.createElement("div");
    const diferencial = Math.max(c.ask - c.bid, 0);
    fill.style.width = `${Math.min(100, (diferencial / maxDiferencial) * 100)}%`;
    bar.appendChild(fill);

    const meta = document.createElement("div");
    meta.className = "exchange-meta";
    meta.innerHTML = `
      <span>Book age <strong>${numero.format(edadMs)} ms</strong></span>
      <span>${escapeHtml(fuente)}</span>
    `;

    art.appendChild(header);
    art.appendChild(precios);
    art.appendChild(bar);
    art.appendChild(meta);
    container.appendChild(art);
  });
}

async function renderJudgeReadiness() {
  const container = $("judgeReadiness");
  if (!container) return;
  const ahora = Date.now();
  if (!preflightCache || ahora - ultimoPreflightMs > 10_000) {
    if (!preflightEnCurso) {
      preflightEnCurso = true;
      fetch("/api/preflight")
        .then((res) => res.ok ? res.json() : null)
        .then((json) => {
          if (json) {
            preflightCache = json;
            ultimoPreflightMs = Date.now();
          }
        })
        .catch(() => {})
        .finally(() => {
          preflightEnCurso = false;
          renderJudgeReadiness();
        });
    }
  }
  const readiness = preflightCache?.judgeReadiness;
  if (!readiness) {
    container.textContent = "Calculando readiness del jurado...";
    return;
  }
  const checks = readiness.checks || [];
  const faltantes = checks.filter((c) => !c.ok).map((c) => c.name);
  container.innerHTML = `
    <div class="judge-score">
      <strong>${readiness.passed}/${readiness.total}</strong>
      <span>${escapeHtml(readiness.status || "review")}</span>
    </div>
    <div class="judge-checks">
      ${checks.map((c) => `<span class="${c.ok ? "ok" : "bad"}">${escapeHtml(c.name)}</span>`).join("")}
    </div>
    <p>${faltantes.length ? `Pendiente: ${escapeHtml(faltantes.join(", "))}` : "Checklist completo: datos live, utilidad neta, fills parciales, wallets, auditoría, riesgo, demo segura y exports."}</p>
  `;
}

function renderLatencias(datos) {
  const container = $("latenciaRanking");
  if (!container) return;
  container.textContent = "";

  const latencias = [...(datos.latenciasExchange || [])]
    .sort((a, b) => (a.promedioMs || 0) - (b.promedioMs || 0))
    .slice(0, 6);
  if (latencias.length === 0) {
    const vacio = document.createElement("p");
    vacio.className = "mini-empty";
    vacio.textContent = "Esperando timestamps de feeds WebSocket.";
    container.appendChild(vacio);
    return;
  }

  latencias.forEach((lat) => {
    const row = document.createElement("div");
    row.className = "latencia-row";
    const estado = (lat.estado || "").includes("alta") ? "mala" : "buena";
    row.innerHTML = `
      <strong>${escapeHtml(lat.exchange)}</strong>
      <span class="${estado}">${formato(lat.promedioMs || 0, 0)} ms</span>
      <small>${escapeHtml(lat.regionSugerida || "iad/us-east")}</small>
    `;
    container.appendChild(row);
  });
}

function iniciarBacktest() {
  const btn = $("btnBacktest");
  if (!btn) return;
  btn.onclick = async () => {
    btn.disabled = true;
    btn.textContent = "Ejecutando...";
    try {
      const res = await fetch("/api/backtest");
      if (res.ok) {
        renderBacktest(await res.json());
      }
    } catch (e) {
      debugError("Error ejecutando backtest", e);
    } finally {
      btn.disabled = false;
      btn.textContent = "Ejecutar";
    }
  };
}

function renderBacktest(datos) {
  const tbody = $("backtestResultados");
  if (!tbody) return;
  tbody.textContent = "";
  [
    ["Base", datos.base],
    ["Optimizada", datos.optimizada],
  ].forEach(([nombre, r]) => {
    const tr = document.createElement("tr");
    [nombre, numero.format(r.tradesEjecutados), dinero.format(r.pnlUsd), `${formato(r.winRate * 100, 1)}%`, dinero.format(r.maxDrawdownUsd), `${formato(r.spreadNetoMedioBps, 2)} bps`]
      .forEach((valor, i) => {
        const td = document.createElement("td");
        td.textContent = valor;
        if (i === 2) td.className = r.pnlUsd >= 0 ? "positivo" : "negativo";
        tr.appendChild(td);
      });
    tbody.appendChild(tr);
  });
}

function renderBalances(datos) {
  const container = $("balances");
  if (!container) return;
  container.textContent = "";

  const ordenados = [...datos.balances].sort((a, b) => a.exchange.localeCompare(b.exchange));
  ordenados.forEach((b) => {
    const div = document.createElement("div");
    div.className = "balance";
    const strong = document.createElement("strong");
    strong.textContent = b.exchange;
    const span = document.createElement("span");
    span.innerHTML = `${dinero.format(b.usd)}<br>${btc.format(b.btc)} BTC`;
    div.appendChild(strong);
    div.appendChild(span);
    container.appendChild(div);
  });
}

function renderConfig(datos) {
  const container = $("configGrid");
  if (!container) return;
  container.textContent = "";

  const c = datos.configuracion;
  const items = [
    { label: "Máx. operación", val: `${btc.format(c.maxOperacionBtc)} BTC` },
    { label: "Diferencial mínimo", val: `${formato(c.minDiferencialNetoBps, 2)} bps` },
    { label: "Deslizamiento", val: `${formato(c.deslizamientoBps, 2)} bps` },
    { label: "Enfriamiento", val: `${c.enfriamientoMs} ms` },
  ];

  items.forEach((item) => {
    const div = document.createElement("div");
    const span = document.createElement("span");
    span.textContent = item.label;
    const strong = document.createElement("strong");
    strong.textContent = item.val;
    div.appendChild(span);
    div.appendChild(strong);
    container.appendChild(div);
  });
}

function renderOportunidades(datos) {
  const tbody = $("oportunidades");
  if (!tbody) return;
  tbody.textContent = "";
  const scorePorRuta = new Map();
  (datos.auditoriaDecisiones || []).forEach((a) => {
    if (!scorePorRuta.has(a.ruta)) {
      scorePorRuta.set(a.ruta, a.score || 0);
    }
  });

  datos.oportunidades.slice(0, 16).forEach((o) => {
    const tr = document.createElement("tr");
    tr.className = o.id === oportunidadSeleccionadaId ? "fila-seleccionada" : "";
    tr.tabIndex = 0;
    tr.addEventListener("click", () => {
      oportunidadSeleccionadaId = o.id;
      renderDetalleOportunidad(ultimoEstado);
      renderOportunidades(ultimoEstado);
    });
    tr.addEventListener("keydown", (evento) => {
      if (evento.key === "Enter" || evento.key === " ") {
        evento.preventDefault();
        tr.click();
      }
    });

    const tdRuta = document.createElement("td");
    tdRuta.textContent = `${o.compraEn} -> ${o.ventaEn}`;

    const tdNeto = document.createElement("td");
    tdNeto.className = o.diferencialNetoBps >= 0 ? "positivo" : "negativo";
    tdNeto.textContent = `${formato(o.diferencialNetoBps, 2)} bps`;

    const tdZScore = document.createElement("td");
    tdZScore.textContent = formato(o.zScore, 2);

    const tdScore = document.createElement("td");
    const score = scorePorRuta.get(`${o.compraEn}->${o.ventaEn}`) || 0;
    tdScore.textContent = score ? formato(score, 3) : "—";

    const tdAmt = document.createElement("td");
    tdAmt.textContent = btc.format(o.cantidadBtc);

    const tdProfit = document.createElement("td");
    tdProfit.textContent = dinero.format(o.utilidadUsd);

    const tdStatus = document.createElement("td");
    const chip = document.createElement("span");
    chip.className = o.ejecutable ? "chip-ok" : "chip-no";
    chip.textContent = o.ejecutable ? "ejecutable" : o.razon;
    tdStatus.appendChild(chip);

    tr.appendChild(tdRuta);
    tr.appendChild(tdNeto);
    tr.appendChild(tdZScore);
    tr.appendChild(tdScore);
    tr.appendChild(tdAmt);
    tr.appendChild(tdProfit);
    tr.appendChild(tdStatus);
    tbody.appendChild(tr);
  });
}

function renderDetalleOportunidad(datos) {
  const panel = $("detalleOportunidad");
  if (!panel) return;
  const oportunidades = datos?.oportunidades || [];
  const seleccionada = oportunidades.find((o) => o.id === oportunidadSeleccionadaId) || oportunidades[0];
  if (!seleccionada) {
    panel.innerHTML = `
      <span class="ceja">Forense</span>
      <strong>Esperando oportunidad</strong>
      <p>Cuando el motor detecte una ruta, aquí aparecerá el desglose neto de costos, liquidez, latencia y decisión.</p>
    `;
    return;
  }
  if (!oportunidadSeleccionadaId) {
    oportunidadSeleccionadaId = seleccionada.id;
  }
  const costos = seleccionada.costos || {};
  const estado = seleccionada.ejecutable ? "Ejecutable" : seleccionada.razon;
  const auditoria = (datos?.auditoriaDecisiones || []).find((a) => a.ruta === `${seleccionada.compraEn}->${seleccionada.ventaEn}`);
  const score = auditoria?.score || 0;
  const decisionCode = auditoria?.decisionCode || seleccionada.decisionCode || "NO_CODE";
  const decisionReason = auditoria?.decisionReason || seleccionada.decisionReason || seleccionada.razon || "";
  const decisionActual = auditoria?.decisionActual ?? seleccionada.decisionActual;
  const decisionThreshold = auditoria?.decisionThreshold ?? seleccionada.decisionThreshold;
  panel.innerHTML = `
    <div class="detalle-header">
      <span class="ceja">Forense</span>
      <strong>${escapeHtml(seleccionada.compraEn)} -> ${escapeHtml(seleccionada.ventaEn)}</strong>
      <span class="${seleccionada.ejecutable ? "chip-ok" : "chip-no"}">${escapeHtml(estado)}</span>
    </div>
    <div class="detalle-grid">
      <div><span>Bruto</span><strong>${formato(seleccionada.diferencialBrutoBps, 2)} bps</strong></div>
      <div><span>Neto</span><strong>${formato(seleccionada.diferencialNetoBps, 2)} bps</strong></div>
      <div><span>Tamaño</span><strong>${btc.format(seleccionada.cantidadBtc)} BTC</strong></div>
      <div><span>Utilidad</span><strong>${dinero.format(seleccionada.utilidadUsd)}</strong></div>
      <div><span>Latencia</span><strong>${seleccionada.latenciaMaxMs} ms</strong></div>
      <div><span>Z-Score</span><strong>${formato(seleccionada.zScore, 2)}</strong></div>
      <div><span>Score EV</span><strong>${score ? formato(score, 3) : "sin score"}</strong></div>
      <div><span>Código</span><strong>${escapeHtml(decisionCode)}</strong></div>
      <div><span>Actual</span><strong>${Number.isFinite(decisionActual) ? formato(decisionActual, 2) : "—"}</strong></div>
      <div><span>Umbral</span><strong>${Number.isFinite(decisionThreshold) ? formato(decisionThreshold, 2) : "—"}</strong></div>
    </div>
    <div class="cost-stack">
      <div><span>Fee compra</span><strong>${dinero.format(costos.feeCompraUsd || 0)}</strong></div>
      <div><span>Fee venta</span><strong>${dinero.format(costos.feeVentaUsd || 0)}</strong></div>
      <div><span>Slippage</span><strong>${dinero.format(costos.deslizamientoUsd || 0)}</strong></div>
      <div><span>Retiro amort.</span><strong>${dinero.format(costos.retiroAmortUsd || 0)}</strong></div>
      <div><span>Riesgo latencia</span><strong>${dinero.format(costos.latenciaRiesgoUsd || 0)}</strong></div>
      <div><span>Total costos</span><strong>${dinero.format(costos.totalUsd || 0)}</strong></div>
    </div>
    <p class="decision-reason">${escapeHtml(decisionReason)}</p>
  `;
}

function renderOperaciones(datos) {
  const tbody = $("operaciones");
  if (!tbody) return;
  tbody.textContent = "";

  datos.operaciones.slice(0, 16).forEach((o) => {
    const tr = document.createElement("tr");

    const tdBuy = document.createElement("td");
    tdBuy.innerHTML = `${escapeHtml(o.compraEn)}<br><span>${dinero.format(o.precioCompra)}</span>`;

    const tdSell = document.createElement("td");
    tdSell.innerHTML = `${escapeHtml(o.ventaEn)}<br><span>${dinero.format(o.precioVenta)}</span>`;

    const tdAmt = document.createElement("td");
    tdAmt.textContent = btc.format(o.cantidadBtc);

    const tdProfit = document.createElement("td");
    tdProfit.className = o.utilidadUsd >= 0 ? "positivo" : "negativo";
    tdProfit.textContent = dinero.format(o.utilidadUsd);

    const tdLat = document.createElement("td");
    tdLat.textContent = `${o.latenciaMaxMs} ms`;

    tr.appendChild(tdBuy);
    tr.appendChild(tdSell);
    tr.appendChild(tdAmt);
    tr.appendChild(tdProfit);
    tr.appendChild(tdLat);
    tbody.appendChild(tr);
  });
}

function renderEventosEjecucion(datos) {
  const tbody = $("eventosEjecucion");
  if (!tbody) return;
  tbody.textContent = "";

  (datos.eventosEjecucion || []).slice(0, 14).forEach((e) => {
    const tr = document.createElement("tr");

    const tdTipo = document.createElement("td");
    const chip = document.createElement("span");
    chip.className = e.severidad === "alta" ? "chip-no" : e.severidad === "media" ? "chip-warn" : "chip-ok";
    chip.textContent = e.tipo;
    tdTipo.appendChild(chip);

    const tdRuta = document.createElement("td");
    tdRuta.textContent = e.ruta;

    const tdDetalle = document.createElement("td");
    tdDetalle.textContent = e.detalle;

    const tdProfit = document.createElement("td");
    tdProfit.className = e.utilidadUsd >= 0 ? "positivo" : "negativo";
    tdProfit.textContent = dinero.format(e.utilidadUsd || 0);

    tr.appendChild(tdTipo);
    tr.appendChild(tdRuta);
    tr.appendChild(tdDetalle);
    tr.appendChild(tdProfit);
    tbody.appendChild(tr);
  });
}

function renderRebalanceos(datos) {
  const tbody = $("rebalanceos");
  if (!tbody) return;
  tbody.textContent = "";

  (datos.rebalanceos || []).slice(0, 14).forEach((r) => {
    const tr = document.createElement("tr");
    const tdActivo = document.createElement("td");
    tdActivo.textContent = r.activo;
    const tdDesde = document.createElement("td");
    tdDesde.textContent = r.desde;
    const tdHacia = document.createElement("td");
    tdHacia.textContent = r.hacia;
    const tdCantidad = document.createElement("td");
    tdCantidad.textContent = r.activo === "BTC" ? `${btc.format(r.cantidad)} BTC` : dinero.format(r.cantidad);
    const tdCosto = document.createElement("td");
    tdCosto.textContent = dinero.format(r.costoUsd || 0);
    tr.appendChild(tdActivo);
    tr.appendChild(tdDesde);
    tr.appendChild(tdHacia);
    tr.appendChild(tdCantidad);
    tr.appendChild(tdCosto);
    tbody.appendChild(tr);
  });
}

function renderAuditoriaDecisiones(datos) {
  const tbody = $("auditoriaDecisiones");
  if (!tbody) return;
  tbody.textContent = "";

  (datos.auditoriaDecisiones || []).slice(0, 18).forEach((a) => {
    const tr = document.createElement("tr");

    const tdRuta = document.createElement("td");
    tdRuta.innerHTML = `${escapeHtml(a.ruta)}<br><span>${escapeHtml(a.par || "")}</span>`;

    const tdDecision = document.createElement("td");
    const chip = document.createElement("span");
    chip.className = a.decision === "candidata_ejecutable" ? "chip-ok" : "chip-no";
    chip.textContent = a.decision === "candidata_ejecutable" ? "acepta" : "descarta";
    tdDecision.appendChild(chip);

    const tdCodigo = document.createElement("td");
    const codigo = document.createElement("code");
    codigo.className = "decision-code";
    codigo.textContent = a.decisionCode || "NO_CODE";
    tdCodigo.appendChild(codigo);

    const tdScore = document.createElement("td");
    tdScore.textContent = formato(a.score || 0, 4);

    const tdCosto = document.createElement("td");
    tdCosto.textContent = dinero.format(a.costoTotalUsd || 0);

    const tdRazon = document.createElement("td");
    const pesos = (a.pesosGa || []).map((p) => formato(p * 100, 0)).join("/");
    const actual = Number.isFinite(a.decisionActual) ? formato(a.decisionActual, 2) : "—";
    const umbral = Number.isFinite(a.decisionThreshold) ? formato(a.decisionThreshold, 2) : "—";
    tdRazon.textContent = `${a.decisionReason || a.razon || "sin razón"} · actual ${actual} / umbral ${umbral} · ${formato(a.diferencialNetoBps || 0, 2)} bps · pesos ${pesos}`;

    tr.appendChild(tdRuta);
    tr.appendChild(tdDecision);
    tr.appendChild(tdCodigo);
    tr.appendChild(tdScore);
    tr.appendChild(tdCosto);
    tr.appendChild(tdRazon);
    tbody.appendChild(tr);
  });
}

function renderGenetico(datos) {
  const g = datos.genetico;
  if (!g) return;

  const genEl = $("gaGeneracion");
  if (genEl) genEl.textContent = `Gen ${g.generacion} · ${g.poblacion} ind.`;
  setText("gaEstado", g.activo ? "Optimizando con historial real" : "Listo para evolucionar");
  setText("gaMuestras", `${numero.format(g.operacionesEvaluadas || 0)} ops · ${numero.format(g.fallosEvaluados || 0)} fallos`);
  setText("gaUmbral", `${formato(g.umbralOptimizado, 2)} bps`);
  setText("gaMaxBtc", `${formato(g.maxOperacionOptimizadaBtc, 3)} BTC`);
  setText("gaLatencia", `${numero.format(g.toleranciaLatenciaMs || 0)} ms`);
  setText("gaMejora", `+${formato(g.mejoraGeneracional || 0, 2)}`);
  setText("gaTemperatura", formato(g.temperaturaAnnealing || 0, 2));
  setText("gaInyecciones", `${numero.format(g.inyeccionesDiferenciales || 0)} DE`);
  setText("gaMetaheuristicas", numero.format((g.metaheuristicas || []).length || 2));

  setText("gaMejorFitness", formato(g.mejorFitness, 2));
  setText("gaFitnessPromedio", formato(g.fitnessPromedio, 2));
  setText("gaDuelo", `Campeón ${formato(g.mejorFitness, 2)} vs Retador ${formato(g.retadorFitness ?? g.fitnessPromedio, 2)}`);
  setText("gaDiversidad", `${formato(g.diversidad * 100, 1)}%`);
  setText("gaTasaMutacion", `${formato(g.tasaMutacion * 100, 1)}%`);
  setText("gaConvergencia", `${formato((1 - g.diversidad) * 100, 1)}%`);

  const pesosContainer = $("gaPesos");
  if (!pesosContainer || !g.mejoresPesos) return;

  const labels = ["Utilidad", "Frescura", "Liquidez", "Confiab.", "Z-Score"];
  const maxPeso = Math.max(...g.mejoresPesos, 0.01);

  pesosContainer.textContent = "";
  g.mejoresPesos.forEach((peso, i) => {
    const div = document.createElement("div");
    div.className = "peso-bar";
    const pct = (peso / maxPeso) * 100;
    div.innerHTML = `
      <span class="peso-label">${labels[i]}</span>
      <div class="peso-track">
        <div class="peso-bar-fill" style="width:${pct}%"></div>
      </div>
      <strong>${formato(peso * 100, 0)}%</strong>
    `;
    pesosContainer.appendChild(div);
  });

  registrarPulsoGa(g);
  dibujarGa(g);
}

function registrarPulsoGa(g) {
  const ultimo = gaHistorial[gaHistorial.length - 1];
  if (ultimo && ultimo.generacion === g.generacion && ultimo.mejor === g.mejorFitness) return;
  gaHistorial.push({
    generacion: g.generacion || 0,
    mejor: g.mejorFitness || 0,
    retador: g.retadorFitness ?? g.fitnessPromedio ?? 0,
    promedio: g.fitnessPromedio || 0,
    diversidad: g.diversidad || 0,
    mejora: g.mejoraGeneracional || 0,
  });
  if (gaHistorial.length > 96) gaHistorial.shift();
}

function renderResumenLlm(datos) {
  const el = $("resumenLlm");
  if (!el) return;
  const mejor = [...(datos.oportunidades || [])].sort((a, b) => b.diferencialNetoBps - a.diferencialNetoBps)[0];
  const ejecutable = (datos.oportunidades || []).find((o) => o.ejecutable);
  const g = datos.genetico;
  const ruta = mejor
    ? `${mejor.compraEn} -> ${mejor.ventaEn} con ${formato(mejor.diferencialNetoBps, 2)} bps netos`
    : "sin rutas suficientes";
  const accion = ejecutable
    ? `hay ruta ejecutable ${ejecutable.compraEn} -> ${ejecutable.ventaEn} por ${dinero.format(ejecutable.utilidadUsd)}`
    : "no hay ruta ejecutable en este ciclo";
  const ga = g
    ? `GA gen ${g.generacion}, fitness ${formato(g.mejorFitness, 2)}, diversidad ${formato(g.diversidad * 100, 1)}%, umbral ${formato(g.umbralOptimizado, 2)} bps`
    : "GA sin estado";
  const persistencia = datos.persistencia?.activa
    ? `SQLite activo: ${numero.format(datos.persistencia.operaciones || 0)} ops, ${numero.format(datos.persistencia.oportunidades || 0)} oportunidades y ${numero.format(datos.persistencia.auditorias || 0)} auditorías persistidas.`
    : "SQLite de auditoría no disponible.";
  el.textContent = [
    `PnL ${dinero.format(datos.metricas.utilidadAcumuladaUsd)}.`,
    `Riesgo: ${datos.metricas.estadoRiesgo}.`,
    `Mejor ruta observada: ${ruta}.`,
    `Decisión actual: ${accion}.`,
    ga,
    persistencia,
    `Latencia promedio ${formato(datos.metricas.latenciaPromedioMs, 0)} ms y ${numero.format(datos.metricas.eventosMercado)} eventos procesados.`
  ].join(" ");
}

function renderExchanges(datos) {
  const container = $("exchangeToggles");
  if (!container) return;

  container.textContent = "";

  const activos = datos.exchangesActivos || {};
  const desdeConfig = Object.keys(datos.configuracion?.exchanges || {});
  const desdeCotizaciones = (datos.cotizaciones || []).map(c => c.exchange);
  const exts = [...new Set([...desdeConfig, ...desdeCotizaciones])].sort();

  exts.forEach(nombre => {
    const div = document.createElement("div");
    div.className = "toggle-exc";
    const label = document.createElement("label");
    label.textContent = nombre;
    const btn = document.createElement("button");
    const estaActivo = activos[nombre] !== false;
    btn.className = `switch-btn ${estaActivo ? "activo" : "inactivo"}`;
    btn.textContent = estaActivo ? "Activo" : "Inactivo";
    btn.dataset.exchange = nombre;
    btn.onclick = async () => {
      const nuevoEstado = btn.classList.contains("activo") ? false : true;
      try {
        const res = await fetch("/api/exchanges", {
          method: "POST",
          headers: headersMutacion({ "Content-Type": "application/json" }),
          body: JSON.stringify({ exchange: nombre, activo: nuevoEstado }),
        });
        if (res.ok) {
          btn.className = `switch-btn ${nuevoEstado ? "activo" : "inactivo"}`;
          btn.textContent = nuevoEstado ? "Activo" : "Inactivo";
          if (ultimoEstado?.exchangesActivos) {
            ultimoEstado.exchangesActivos[nombre] = nuevoEstado;
          }
        }
      } catch (e) {
        debugError("Error toggling exchange", e);
      }
    };
    div.appendChild(label);
    div.appendChild(btn);
    container.appendChild(div);
  });
}

function setText(id, val) {
  const el = $(id);
  if (el) el.textContent = val;
}

function escapeHtml(valor) {
  return String(valor ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function detectarNotificaciones(datos) {
  datos.oportunidades.forEach((o) => {
    if (o.ejecutable && o.utilidadUsd > 50 && !opsNotificadas.has(o.id)) {
      opsNotificadas.add(o.id);
      lanzarNotificacion(o);
    }
  });
}

function lanzarNotificacion(o) {
  const container = $("notificaciones");
  if (!container) return;

  const div = document.createElement("div");
  div.className = "notificacion";
  
  const title = document.createElement("strong");
  title.style.color = "var(--verde)";
  title.textContent = "⚡ ¡Oportunidad de Arbitraje!";
  
  const route = document.createElement("span");
  route.innerHTML = `Ruta: <strong>${escapeHtml(o.compraEn)} -> ${escapeHtml(o.ventaEn)}</strong>`;
  
  const profit = document.createElement("span");
  profit.innerHTML = `Utilidad estimada: <strong>${dinero.format(o.utilidadUsd)}</strong> (${formato(o.diferencialNetoBps, 2)} bps)`;

  div.appendChild(title);
  div.appendChild(route);
  div.appendChild(profit);
  container.appendChild(div);

  // Auto-dismiss
  setTimeout(() => {
    div.style.animation = "slideIn 0.3s cubic-bezier(0.16, 1, 0.3, 1) reverse forwards";
    setTimeout(() => {
      div.remove();
    }, 300);
  }, 4000);
}

function dibujarMapa(datos) {
  const canvas = $("canvasMapa");
  if (!canvas) return;
  const ctx = prepararCanvas(canvas);
  const w = canvas._anchoLogico;
  const h = canvas._altoLogico;
  ctx.clearRect(0, 0, w, h);

  const temaOscuro = document.documentElement.getAttribute("data-theme") === "dark";
  const colorTinta = temaOscuro ? "#f4f0e6" : "#11110f";
  const colorFondo = temaOscuro ? "#191813" : "#fffaf0";
  const colorMuted = temaOscuro ? "#b8b0a0" : "#625d52";

  fondoArquitectonico(ctx, w, h, temaOscuro);

  const exchanges = datos.cotizaciones.map((c) => c.exchange);
  const centroX = w * 0.5;
  const centroY = h * 0.52;
  const radioX = w * 0.34;
  const radioY = h * 0.32;
  const posiciones = new Map();

  exchanges.forEach((nombre, i) => {
    const angulo = -Math.PI / 2 + (Math.PI * 2 * i) / Math.max(exchanges.length, 1);
    posiciones.set(nombre, {
      x: centroX + Math.cos(angulo) * radioX,
      y: centroY + Math.sin(angulo) * radioY,
    });
  });

  datos.oportunidades.slice(0, 18).forEach((o, i) => {
    const a = posiciones.get(o.compraEn);
    const b = posiciones.get(o.ventaEn);
    if (!a || !b) return;
    const fuerza = Math.max(0.18, Math.min(1, o.diferencialNetoBps / 8));
    ctx.strokeStyle = o.ejecutable ? `rgba(38,208,124,${0.32 + fuerza * 0.55})` : `rgba(255,91,63,${0.18 + fuerza * 0.3})`;
    ctx.lineWidth = o.ejecutable ? 2.4 + fuerza * 5 : 1.2;
    ctx.beginPath();
    const dx = b.x - a.x;
    const dy = b.y - a.y;
    ctx.moveTo(a.x, a.y);
    ctx.bezierCurveTo(a.x + dy * 0.14, a.y - dx * 0.14, b.x + dy * 0.14, b.y - dx * 0.14, b.x, b.y);
    ctx.stroke();

    if (i < 5 && o.ejecutable) {
      ctx.fillStyle = "#dfff43";
      const t = (Date.now() / 900 + i * 0.18) % 1;
      ctx.beginPath();
      // Interpolación sobre curva bezier simple
      const x = a.x + (b.x - a.x) * t;
      const y = a.y + (b.y - a.y) * t;
      ctx.arc(x, y, 5, 0, Math.PI * 2);
      ctx.fill();
    }
  });

  datos.cotizaciones.forEach((c) => {
    const p = posiciones.get(c.exchange);
    if (!p) return;
    ctx.fillStyle = colorFondo;
    ctx.strokeStyle = colorTinta;
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.rect(p.x - 56, p.y - 30, 112, 60);
    ctx.fill();
    ctx.stroke();
    ctx.fillStyle = colorTinta;
    ctx.font = "700 17px Archivo, sans-serif";
    ctx.textAlign = "center";
    ctx.fillText(c.exchange, p.x, p.y - 4);
    ctx.fillStyle = colorMuted;
    ctx.font = "700 12px Archivo, sans-serif";
    ctx.fillText(`${formato(c.ask - c.bid, 2)} dif.`, p.x, p.y + 17);
  });
}

function dibujarSeries(datos) {
  const canvas = $("canvasSeries");
  if (!canvas) return;
  const ctx = prepararCanvas(canvas);
  const w = canvas._anchoLogico;
  const h = canvas._altoLogico;
  ctx.clearRect(0, 0, w, h);

  const temaOscuro = document.documentElement.getAttribute("data-theme") === "dark";
  const colorTinta = temaOscuro ? "#f4f0e6" : "#11110f";

  fondoArquitectonico(ctx, w, h, temaOscuro);
  
  const pnlColor = temaOscuro ? "#26d07c" : "#0c8a55";
  const difColor = temaOscuro ? "#4eb3ff" : "#1769aa";
  
  dibujarLinea(ctx, datos.seriePnl.map((p) => p.valor), pnlColor, w, h, 0.58);
  dibujarLinea(ctx, datos.serieDiferencial.map((p) => p.valor), difColor, w, h, 0.34);

  // Agregar ejes y etiquetas
  ctx.fillStyle = colorTinta;
  ctx.font = "800 14px Archivo, sans-serif";
  ctx.textAlign = "left";
  ctx.fillText("Ganancia/pérdida acumulada (USD)", 24, 30);
  
  ctx.fillStyle = difColor;
  ctx.fillText("Diferencial neto (bps)", 24, 52);
}

function dibujarGa(g) {
  const canvas = $("canvasGa");
  if (!canvas) return;
  const ctx = prepararCanvas(canvas);
  const w = canvas._anchoLogico;
  const h = canvas._altoLogico;
  ctx.clearRect(0, 0, w, h);

  const temaOscuro = document.documentElement.getAttribute("data-theme") === "dark";
  ctx.fillStyle = temaOscuro ? "#090a08" : "#fff7e6";
  ctx.fillRect(0, 0, w, h);
  ctx.strokeStyle = temaOscuro ? "rgba(247,147,26,0.18)" : "rgba(217,120,5,0.25)";
  ctx.lineWidth = 1;
  for (let x = 20; x < w; x += 46) {
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x + 32, h);
    ctx.stroke();
  }
  for (let y = 28; y < h; y += 42) {
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(w, y);
    ctx.stroke();
  }

  if (g && gaHistorial.length === 0) registrarPulsoGa(g);
  const datos = gaHistorial.length > 1 ? gaHistorial : [
    { generacion: 0, mejor: 0, promedio: 0, diversidad: 1, mejora: 0 },
    { generacion: 1, mejor: g?.mejorFitness || 0, retador: g?.retadorFitness ?? g?.fitnessPromedio ?? 0, promedio: g?.fitnessPromedio || 0, diversidad: g?.diversidad || 1, mejora: g?.mejoraGeneracional || 0 },
  ];

  const valores = datos.flatMap((p) => [p.mejor, p.retador ?? p.promedio, p.promedio]);
  const min = Math.min(...valores, 0);
  const max = Math.max(...valores, 1);
  const rango = Math.max(max - min, 1);
  const px = (i) => 34 + (i / Math.max(datos.length - 1, 1)) * (w - 68);
  const py = (valor) => h - 34 - ((valor - min) / rango) * (h - 72);

  trazarSerieGa(ctx, datos.map((p) => p.mejor), px, py, "#f8c547", 4);
  trazarSerieGa(ctx, datos.map((p) => p.retador ?? p.promedio), px, py, "#fb7185", 3);
  trazarSerieGa(ctx, datos.map((p) => p.promedio), px, py, "#22d3ee", 3);

  datos.slice(-18).forEach((p, i, arr) => {
    const x = px(datos.length - arr.length + i);
    const y = h - 20 - p.diversidad * 42;
    ctx.fillStyle = `rgba(32, 230, 154, ${0.22 + p.diversidad * 0.55})`;
    ctx.fillRect(x - 3, y, 6, Math.max(8, p.diversidad * 42));
  });

  const ultimo = datos[datos.length - 1];
  ctx.fillStyle = temaOscuro ? "#fff7df" : "#12100b";
  ctx.font = "900 13px Archivo, sans-serif";
  ctx.textAlign = "left";
  ctx.fillText(`Auto Gen ${ultimo.generacion} · campeón ${formato(ultimo.mejor, 2)} · retador ${formato(ultimo.retador ?? ultimo.promedio, 2)}`, 22, 24);
  ctx.fillStyle = "#f8c547";
  ctx.fillText("campeón", 22, h - 14);
  ctx.fillStyle = "#fb7185";
  ctx.fillText("retador", 112, h - 14);
  ctx.fillStyle = "#22d3ee";
  ctx.fillText("promedio", 196, h - 14);
  ctx.fillStyle = "#20e69a";
  ctx.fillText("diversidad", 296, h - 14);
}

function trazarSerieGa(ctx, valores, px, py, color, grosor) {
  ctx.strokeStyle = color;
  ctx.lineWidth = grosor;
  ctx.lineJoin = "round";
  ctx.lineCap = "round";
  ctx.beginPath();
  valores.forEach((valor, i) => {
    const x = px(i);
    const y = py(valor);
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  });
  ctx.stroke();
  const x = px(valores.length - 1);
  const y = py(valores[valores.length - 1]);
  ctx.fillStyle = color;
  ctx.beginPath();
  ctx.arc(x, y, 5, 0, Math.PI * 2);
  ctx.fill();
}

function dibujarLinea(ctx, valores, color, w, h, base) {
  if (valores.length < 2) return;
  const min = Math.min(...valores, 0);
  const max = Math.max(...valores, 1);
  const rango = Math.max(max - min, 1);
  ctx.strokeStyle = color;
  ctx.lineWidth = 3;
  ctx.beginPath();
  valores.forEach((valor, i) => {
    const x = 28 + (i / (valores.length - 1)) * (w - 56);
    const y = h * base - ((valor - min) / rango) * h * 0.24;
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  });
  ctx.stroke();
}

function fondoArquitectonico(ctx, w, h, temaOscuro) {
  ctx.fillStyle = temaOscuro ? "#191813" : "#fffaf0";
  ctx.fillRect(0, 0, w, h);
  
  ctx.strokeStyle = temaOscuro ? "#413d34" : "#cfc4ad";
  ctx.lineWidth = 1;
  for (let x = 0; x < w; x += 52) {
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, h);
    ctx.stroke();
  }
  for (let y = 0; y < h; y += 44) {
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(w, y);
    ctx.stroke();
  }
}

function prepararCanvas(canvas) {
  const ratio = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  const ancho = Math.max(320, Math.floor(rect.width));
  const alto = Math.max(220, Math.floor(rect.height));
  const anchoFisico = Math.floor(ancho * ratio);
  const altoFisico = Math.floor(alto * ratio);
  if (canvas.width !== anchoFisico || canvas.height !== altoFisico) {
    canvas.width = anchoFisico;
    canvas.height = altoFisico;
  }
  const ctx = canvas.getContext("2d");
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
  canvas._anchoLogico = ancho;
  canvas._altoLogico = alto;
  return ctx;
}

function mejorDiferencial(datos) {
  const mejor = datos.oportunidades.reduce((acc, o) => Math.max(acc, o.diferencialNetoBps), 0);
  return `${formato(mejor, 2)} bps`;
}

function formato(valor, decimales) {
  return Number(valor || 0).toLocaleString("es-MX", {
    minimumFractionDigits: decimales,
    maximumFractionDigits: decimales,
  });
}
