const estado = {
  datos: null,
  ultimoMensaje: 0,
};

const $ = (id) => document.getElementById(id);
const dinero = new Intl.NumberFormat("es-MX", {
  style: "currency",
  currency: "USD",
  maximumFractionDigits: 2,
});
const numero = new Intl.NumberFormat("es-MX", { maximumFractionDigits: 2 });
const btc = new Intl.NumberFormat("es-MX", { maximumFractionDigits: 6 });

conectar();
setInterval(verificarConexion, 900);

async function conectar() {
  const protocolo = location.protocol === "https:" ? "wss" : "ws";
  const socket = new WebSocket(`${protocolo}://${location.host}/tiempo-real`);
  cambiarSocket("conectando");

  socket.addEventListener("open", () => cambiarSocket("en vivo", true));
  socket.addEventListener("message", (evento) => {
    estado.datos = JSON.parse(evento.data);
    estado.ultimoMensaje = Date.now();
    renderizar(estado.datos);
  });
  socket.addEventListener("close", () => {
    cambiarSocket("reconectando");
    setTimeout(conectar, 900);
  });
  socket.addEventListener("error", () => cambiarSocket("sin enlace", false));
}

function verificarConexion() {
  if (!estado.ultimoMensaje) return;
  const viejo = Date.now() - estado.ultimoMensaje > 2400;
  if (viejo) cambiarSocket("sin datos", false);
}

function cambiarSocket(texto, ok) {
  const el = $("estadoSocket");
  el.classList.toggle("ok", ok === true);
  el.classList.toggle("error", ok === false);
  el.lastChild.nodeValue = ` ${texto}`;
}

function renderizar(datos) {
  $("pnl").textContent = dinero.format(datos.metricas.utilidadAcumuladaUsd);
  $("retorno").textContent = `${formato(datos.metricas.retornoBps, 2)} bps`;
  $("eventos").textContent = numero.format(datos.metricas.eventosMercado);
  $("latencia").textContent = `${formato(datos.metricas.latenciaPromedioMs, 0)} ms`;
  $("riesgo").textContent = datos.metricas.estadoRiesgo;
  $("trabajadores").textContent = `${datos.metricas.trabajadores} trabajadores`;
  $("mejorSpread").textContent = mejorSpread(datos);

  renderMercado(datos);
  renderBalances(datos);
  renderConfig(datos);
  renderOportunidades(datos);
  renderOperaciones(datos);
  dibujarMapa(datos);
  dibujarSeries(datos);
}

function renderMercado(datos) {
  const spreads = datos.cotizaciones.map((c) => c.ask - c.bid).filter((s) => s > 0);
  const maxSpread = Math.max(...spreads, 1);
  $("exchangeLista").innerHTML = datos.cotizaciones
    .map((c) => {
      const spread = Math.max(c.ask - c.bid, 0);
      const ancho = Math.min(100, (spread / maxSpread) * 100);
      return `
        <article class="exchange">
          <header>
            <h3>${c.exchange}</h3>
            <span class="latencia-chip">${c.latenciaMs || 0} ms</span>
          </header>
          <div class="precios">
            <div class="precio bid"><span>Compra</span><strong>${dinero.format(c.bid)}</strong></div>
            <div class="precio ask"><span>Venta</span><strong>${dinero.format(c.ask)}</strong></div>
          </div>
          <div class="barra-spread" aria-label="Diferencial">
            <div style="width:${ancho}%"></div>
          </div>
        </article>
      `;
    })
    .join("");
}

function renderBalances(datos) {
  $("balances").innerHTML = datos.balances
    .sort((a, b) => a.exchange.localeCompare(b.exchange))
    .map(
      (b) => `
        <div class="balance">
          <strong>${b.exchange}</strong>
          <span>${dinero.format(b.usd)}<br>${btc.format(b.btc)} BTC</span>
        </div>
      `,
    )
    .join("");
}

function renderConfig(datos) {
  const c = datos.configuracion;
  $("configGrid").innerHTML = `
    <div><span>Máx. operación</span><strong>${btc.format(c.maxOperacionBtc)} BTC</strong></div>
    <div><span>Diferencial mínimo</span><strong>${formato(c.minSpreadNetoBps, 2)} bps</strong></div>
    <div><span>Deslizamiento</span><strong>${formato(c.slippageBps, 2)} bps</strong></div>
    <div><span>Enfriamiento</span><strong>${c.cooldownMs} ms</strong></div>
  `;
}

function renderOportunidades(datos) {
  $("oportunidades").innerHTML = datos.oportunidades
    .slice(0, 16)
    .map(
      (o) => `
      <tr>
        <td>${o.compraEn} -> ${o.ventaEn}</td>
        <td class="${o.spreadNetoBps >= 0 ? "positivo" : "negativo"}">${formato(o.spreadNetoBps, 2)} bps</td>
        <td>${btc.format(o.cantidadBtc)}</td>
        <td>${dinero.format(o.utilidadUsd)}</td>
        <td><span class="${o.ejecutable ? "chip-ok" : "chip-no"}">${o.ejecutable ? "ejecutable" : o.razon}</span></td>
      </tr>
    `,
    )
    .join("");
}

function renderOperaciones(datos) {
  $("operaciones").innerHTML = datos.operaciones
    .slice(0, 16)
    .map(
      (o) => `
      <tr>
        <td>${o.compraEn}<br><span>${dinero.format(o.precioCompra)}</span></td>
        <td>${o.ventaEn}<br><span>${dinero.format(o.precioVenta)}</span></td>
        <td>${btc.format(o.cantidadBtc)}</td>
        <td class="${o.utilidadUsd >= 0 ? "positivo" : "negativo"}">${dinero.format(o.utilidadUsd)}</td>
        <td>${o.latenciaMaxMs} ms</td>
      </tr>
    `,
    )
    .join("");
}

function dibujarMapa(datos) {
  const canvas = $("canvasMapa");
  const ctx = prepararCanvas(canvas);
  const w = canvas._anchoLogico;
  const h = canvas._altoLogico;
  ctx.clearRect(0, 0, w, h);

  fondoArquitectonico(ctx, w, h);
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
    const fuerza = Math.max(0.18, Math.min(1, o.spreadNetoBps / 8));
    ctx.strokeStyle = o.ejecutable ? `rgba(24,119,78,${0.22 + fuerza * 0.55})` : `rgba(185,63,44,${0.12 + fuerza * 0.26})`;
    ctx.lineWidth = o.ejecutable ? 2.4 + fuerza * 5 : 1.2;
    ctx.beginPath();
    const dx = b.x - a.x;
    const dy = b.y - a.y;
    ctx.moveTo(a.x, a.y);
    ctx.bezierCurveTo(a.x + dy * 0.14, a.y - dx * 0.14, b.x + dy * 0.14, b.y - dx * 0.14, b.x, b.y);
    ctx.stroke();

    if (i < 5 && o.ejecutable) {
      ctx.fillStyle = "rgba(24,119,78,0.9)";
      const t = (Date.now() / 900 + i * 0.18) % 1;
      ctx.beginPath();
      ctx.arc(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t, 4.5, 0, Math.PI * 2);
      ctx.fill();
    }
  });

  datos.cotizaciones.forEach((c) => {
    const p = posiciones.get(c.exchange);
    if (!p) return;
    ctx.fillStyle = "#fbfaf5";
    ctx.strokeStyle = "#171915";
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.rect(p.x - 56, p.y - 30, 112, 60);
    ctx.fill();
    ctx.stroke();
    ctx.fillStyle = "#171915";
    ctx.font = "700 17px Archivo, sans-serif";
    ctx.textAlign = "center";
    ctx.fillText(c.exchange, p.x, p.y - 4);
    ctx.fillStyle = "#6a675e";
    ctx.font = "700 12px Archivo, sans-serif";
    ctx.fillText(`${formato(c.ask - c.bid, 2)} dif.`, p.x, p.y + 17);
  });
}

function dibujarSeries(datos) {
  const canvas = $("canvasSeries");
  const ctx = prepararCanvas(canvas);
  const w = canvas._anchoLogico;
  const h = canvas._altoLogico;
  ctx.clearRect(0, 0, w, h);
  fondoArquitectonico(ctx, w, h);
  dibujarLinea(ctx, datos.seriePnl.map((p) => p.valor), "#18774e", w, h, 0.58);
  dibujarLinea(ctx, datos.serieSpread.map((p) => p.valor), "#315f93", w, h, 0.34);

  ctx.fillStyle = "#171915";
  ctx.font = "800 15px Archivo, sans-serif";
  ctx.fillText("Ganancia/pérdida acumulada", 24, 34);
  ctx.fillStyle = "#315f93";
  ctx.fillText("Diferencial neto", 24, 58);
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

function fondoArquitectonico(ctx, w, h) {
  ctx.fillStyle = "#fffdf7";
  ctx.fillRect(0, 0, w, h);
  ctx.strokeStyle = "rgba(23,25,21,0.08)";
  ctx.lineWidth = 1;
  for (let x = 0; x < w; x += 44) {
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x + w * 0.08, h);
    ctx.stroke();
  }
  for (let y = 0; y < h; y += 38) {
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

function mejorSpread(datos) {
  const mejor = datos.oportunidades.reduce((acc, o) => Math.max(acc, o.spreadNetoBps), 0);
  return `${formato(mejor, 2)} bps`;
}

function formato(valor, decimales) {
  return Number(valor || 0).toLocaleString("es-MX", {
    minimumFractionDigits: decimales,
    maximumFractionDigits: decimales,
  });
}
