const $ = (id) => document.getElementById(id);
const start = $("start");
const stop = $("stop");
const run = $("run");
const loadWindow = $("loadWindow");
const windowMinutes = $("windowMinutes");

function mutationHeaders() {
  const headers = { "Content-Type": "application/json" };
  const token = localStorage.getItem("mayabAdminToken");
  if (token) headers.Authorization = `Bearer ${token}`;
  return headers;
}

async function request(url, payload = {}) {
  const response = await fetch(url, {
    method: "POST",
    headers: mutationHeaders(),
    body: JSON.stringify(payload),
  });
  const body = await response.json().catch(() => ({}));
  if (!response.ok || body.ok === false) throw new Error(body.error || `HTTP ${response.status}`);
  return body;
}

function renderState(state) {
  const snapshotsSeleccionados = Number(state.snapshots || 0);
  const snapshotsRecientes = Number(state.historialVentanaPredeterminadaSnapshots || 0);
  const historialTotal = Number(state.historialSnapshots || 0);
  const snapshotsDisponibles = snapshotsSeleccionados || snapshotsRecientes;
  const duracion = snapshotsSeleccionados
    ? Number(state.duracionSegundos || 0)
    : Math.min(Number(state.historialDuracionSegundos || 0), 600);
  $("snapshots").textContent = snapshotsDisponibles.toLocaleString("es-MX");
  $("duration").textContent = duracion >= 60
    ? `${Math.floor(duracion / 60)} min ${duracion % 60} s`
    : `${duracion} s`;
  $("dot").classList.toggle("active", state.activa === true);
  $("status").textContent = state.activa
    ? "Capturando cotizaciones públicas"
    : snapshotsSeleccionados > 0
      ? "Ventana seleccionada lista para replay"
      : snapshotsRecientes > 0 ? "Últimos 10 minutos listos" : "Listo para capturar";
  $("historyStatus").textContent = historialTotal > 0
    ? `${historialTotal.toLocaleString("es-MX")} muestras disponibles · hasta 60 min`
    : "Esperando las primeras cotizaciones públicas";
  start.disabled = state.activa;
  stop.disabled = !state.activa;
  loadWindow.disabled = state.activa || historialTotal === 0;
  windowMinutes.disabled = state.activa || historialTotal === 0;
  run.disabled = state.activa || snapshotsDisponibles === 0;
}

async function refresh() {
  try {
    const response = await fetch("/api/replay/captura/estado");
    if (response.ok) renderState(await response.json());
  } catch { $("status").textContent = "No se pudo consultar el servidor"; }
}

start.onclick = async () => {
  start.disabled = true;
  $("status").textContent = "Iniciando captura…";
  try { await request("/api/replay/captura/iniciar"); await refresh(); }
  catch (error) { $("status").textContent = error.message; start.disabled = false; }
};
stop.onclick = async () => {
  stop.disabled = true;
  $("status").textContent = "Cerrando tape…";
  try { await request("/api/replay/captura/detener"); await refresh(); }
  catch (error) { $("status").textContent = error.message; stop.disabled = false; }
};
loadWindow.onclick = async () => {
  loadWindow.disabled = true;
  $("status").textContent = "Preparando ventana de mercado…";
  try {
    const result = await request("/api/replay/captura/ventana", {
      minutos: Number(windowMinutes.value),
    });
    $("status").textContent = `${Number(result.snapshots).toLocaleString("es-MX")} snapshots listos`;
    await refresh();
  } catch (error) {
    $("status").textContent = error.message;
  } finally {
    loadWindow.disabled = false;
  }
};
run.onclick = async () => {
  run.disabled = true;
  $("resultTitle").textContent = "Ejecutando motor aislado…";
  try {
    const result = await request("/api/replay/ejecutar");
    $("resultTitle").textContent = "Replay completado";
    $("resultGrid").classList.remove("muted");
    const hash = typeof result.inputSha256 === "string" ? result.inputSha256 : "sin-huella";
    $("resultGrid").innerHTML = `<article><span>Ticks</span><strong>${Number(result.ticksProcesados).toLocaleString("es-MX")}</strong></article><article><span>Operaciones</span><strong>${Number(result.operaciones).toLocaleString("es-MX")}</strong></article><article><span>PnL simulado</span><strong>$${Number(result.pnlUsd).toLocaleString("es-MX", {minimumFractionDigits: 2, maximumFractionDigits: 2})}</strong></article><article title="${hash}"><span>Input SHA-256</span><strong>${hash.slice(0, 12)}…</strong></article>`;
    const fuente = result.fuente === "historial_publico_ultimos_10_min"
      ? "últimos 10 min disponibles"
      : "ventana elegida";
    $("resultNote").textContent = `${result.mensaje} · ${fuente} · reloj del tape · adversidad aleatoria desactivada`;
  } catch (error) { $("resultTitle").textContent = "No se pudo ejecutar"; $("resultNote").textContent = error.message; }
  finally { await refresh(); }
};

function installMotion() {
  if (matchMedia("(prefers-reduced-motion: reduce)").matches || !matchMedia("(hover: hover) and (pointer: fine)").matches) return;
  document.querySelectorAll(".replay-card").forEach((card) => {
    card.addEventListener("pointermove", (event) => {
      const rect = card.getBoundingClientRect();
      const x = (event.clientX - rect.left) / rect.width - 0.5;
      const y = (event.clientY - rect.top) / rect.height - 0.5;
      card.style.setProperty("--replay-tilt-x", `${(-y * 2.4).toFixed(2)}deg`);
      card.style.setProperty("--replay-tilt-y", `${(x * 2.4).toFixed(2)}deg`);
    });
    card.addEventListener("pointerleave", () => {
      card.style.setProperty("--replay-tilt-x", "0deg");
      card.style.setProperty("--replay-tilt-y", "0deg");
    });
  });
  const page = $("replay-top");
  page.addEventListener("pointermove", (event) => {
    const x = event.clientX / window.innerWidth - 0.5;
    const y = event.clientY / window.innerHeight - 0.5;
    page.style.setProperty("--hero-shift-x", `${(x * 7).toFixed(1)}px`);
    page.style.setProperty("--hero-shift-y", `${(y * 7).toFixed(1)}px`);
  });
}

refresh();
setInterval(refresh, 2000);
installMotion();
