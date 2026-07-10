// Animación del Favicon - Mayab Arbitraje BTC
const DEBUG_ACTIVO =
  new URLSearchParams(location.search).has("debug") ||
  localStorage.getItem("mayabDebug") === "1";

const BTC_PATH_STR = "M360-120v-80H240v-80h80v-400h-80v-80h120v-80h80v80h80v-80h80v85q52 14 86 56.5t34 98.5q0 29-10 55.5T682-497q35 21 56.5 57t21.5 80q0 66-47 113t-113 47v80h-80v-80h-80v80h-80Zm40-400h160q33 0 56.5-23.5T640-600q0-33-23.5-56.5T560-680H400v160Zm0 240h200q33 0 56.5-23.5T680-360q0-33-23.5-56.5T600-440H400v160Z";

class FaviconAnimator {
  constructor() {
    this.linkEl = document.getElementById("favicon");
    if (!this.linkEl) {
      // Fallback selector if ID is not ready
      this.linkEl = document.querySelector("link[rel~='icon']");
    }
    
    this.canvas = document.createElement("canvas");
    this.canvas.width = 32;
    this.canvas.height = 32;
    this.ctx = this.canvas.getContext("2d");
    this.btcPath = new Path2D(BTC_PATH_STR);

    // States
    this.angulo = 0;
    this.velocidadRotacion = 0.04;
    this.estadoSocket = "conectando";
    this.socketOk = undefined;
    this.ultimoArbitrajeMs = 0;
    this.intensidadFlash = 0;
    this.timerId = null;
    this.fps = 15;
    this.intervalMs = 1000 / this.fps;

    this.init();
  }

  init() {
    // Escuchar eventos globales del dashboard
    window.addEventListener("mayab:socket", (e) => {
      this.estadoSocket = e.detail.texto;
      this.socketOk = e.detail.ok;
    });

    window.addEventListener("mayab:arbitraje", () => {
      this.ultimoArbitrajeMs = Date.now();
      this.intensidadFlash = 1.0;
    });

    // Optimización de visibilidad
    document.addEventListener("visibilitychange", () => {
      this.actualizarBucle();
    });

    this.actualizarBucle();

    if (DEBUG_ACTIVO) {
      console.log("[Favicon] Animador de favicon inicializado y escuchando eventos.");
    }
  }

  actualizarBucle() {
    if (this.timerId) {
      clearInterval(this.timerId);
      this.timerId = null;
    }

    if (document.hidden) {
      // Si la pestaña está oculta, bajar frecuencia drásticamente para ahorrar recursos
      this.timerId = setInterval(() => this.tick(), 2000);
    } else {
      // Frecuencia normal de 15 FPS para una animación fluida pero eficiente
      this.timerId = setInterval(() => this.tick(), this.intervalMs);
    }
  }

  tick() {
    this.update();
    this.render();
  }

  update() {
    const now = Date.now();
    
    // Decaimiento del destello de arbitraje (1.5 segundos de duración)
    const timeSinceArbitraje = now - this.ultimoArbitrajeMs;
    if (timeSinceArbitraje < 1500) {
      this.intensidadFlash = 1.0 - (timeSinceArbitraje / 1500);
    } else {
      this.intensidadFlash = 0;
    }

    // Calcular velocidad meta de rotación según estado de conexión
    let targetVel = 0.04; // Conectado
    if (this.socketOk === false) {
      targetVel = 0; // Detener rotación si no hay enlace/datos
    } else if (this.socketOk === undefined || this.estadoSocket === "conectando" || this.estadoSocket === "reconectando") {
      targetVel = 0.015; // Rotación muy lenta
    }

    // Sumar impulso de velocidad si hay destello por arbitraje reciente
    if (this.intensidadFlash > 0) {
      this.velocidadRotacion = targetVel + (0.35 * this.intensidadFlash);
    } else {
      this.velocidadRotacion = targetVel;
    }

    this.angulo += this.velocidadRotacion;
  }

  render() {
    const ctx = this.ctx;
    const now = Date.now();
    ctx.clearRect(0, 0, 32, 32);

    // Calcular pulso sinusoidal para efectos visuales (ciclo de 2 segundos)
    const pulsePhase = (now % 2000) / 2000;
    const pulseValue = Math.sin(pulsePhase * Math.PI * 2) * 0.5 + 0.5;

    // Determinar color de base según estado de conexión
    let r, g, b;
    if (this.socketOk === false) {
      // Error: Rojo
      r = 239; g = 68; b = 68;
    } else if (this.socketOk === undefined || this.estadoSocket === "conectando" || this.estadoSocket === "reconectando") {
      // Transición/Cargando: Naranja/Ambar
      r = 245; g = 158; b = 11;
    } else {
      // Correcto: Verde Esmeralda
      r = 16; g = 185; b = 129;
    }

    // Mezclar con color oro brillante (251, 191, 36) si hay flash de arbitraje activo
    if (this.intensidadFlash > 0) {
      r = Math.round(r * (1 - this.intensidadFlash) + 251 * this.intensidadFlash);
      g = Math.round(g * (1 - this.intensidadFlash) + 191 * this.intensidadFlash);
      b = Math.round(b * (1 - this.intensidadFlash) + 36 * this.intensidadFlash);
    }

    // 1. Dibujar brillo de fondo (glow central)
    let glowAlpha = 0.08 + 0.06 * pulseValue;
    if (this.intensidadFlash > 0) {
      glowAlpha = glowAlpha * (1 - this.intensidadFlash) + 0.4 * this.intensidadFlash;
    }
    ctx.beginPath();
    ctx.arc(16, 16, 11, 0, Math.PI * 2);
    ctx.fillStyle = `rgba(${r}, ${g}, ${b}, ${glowAlpha})`;
    ctx.fill();

    // 2. Dibujar anillo exterior indicador
    let ringAlpha = 0.4 + 0.3 * pulseValue;
    if (this.intensidadFlash > 0) {
      ringAlpha = ringAlpha * (1 - this.intensidadFlash) + 0.9 * this.intensidadFlash;
    }
    ctx.beginPath();
    ctx.arc(16, 16, 14, 0, Math.PI * 2);
    ctx.strokeStyle = `rgba(${r}, ${g}, ${b}, ${ringAlpha})`;
    ctx.lineWidth = 1.5;
    ctx.stroke();

    // 3. Dibujar símbolo de Bitcoin (BTC) rotado y escalado
    ctx.save();
    ctx.translate(16, 16);
    ctx.rotate(this.angulo);
    
    // Escala del símbolo (aprox 20px de alto en caja de 960px)
    const scale = 20 / 960;
    ctx.scale(scale, scale);
    ctx.translate(-480, 480);

    // Color del símbolo Bitcoin
    let btcFill;
    if (this.socketOk === false) {
      btcFill = "#64748b"; // Gris si está desconectado
    } else {
      // Mezclar naranja Bitcoin tradicional (#F7931A) con amarillo oro brillante durante flash
      if (this.intensidadFlash > 0) {
        const rBtc = Math.round(247 * (1 - this.intensidadFlash) + 251 * this.intensidadFlash);
        const gBtc = Math.round(147 * (1 - this.intensidadFlash) + 191 * this.intensidadFlash);
        const bBtc = Math.round(26 * (1 - this.intensidadFlash) + 36 * this.intensidadFlash);
        btcFill = `rgb(${rBtc}, ${gBtc}, ${bBtc})`;
      } else {
        btcFill = "#f7931a"; // Naranja standard
      }
    }

    ctx.fillStyle = btcFill;
    ctx.fill(this.btcPath);
    ctx.restore();

    // 4. Actualizar favicon en la cabecera
    if (this.linkEl) {
      this.linkEl.href = this.canvas.toDataURL("image/png");
    }
  }
}

// Iniciar cuando el DOM esté listo
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => new FaviconAnimator());
} else {
  new FaviconAnimator();
}
