if (window.matchMedia('(pointer: fine)').matches) {
  initCursor();
}

function initCursor() {
  const cursor = document.createElement('span');
  cursor.className = 'custom-cursor';
  cursor.setAttribute('aria-hidden', 'true');
  cursor.hidden = true;

  const cursorIcon = document.createElement('div');
  cursorIcon.className = 'cursor-icon';
  cursor.appendChild(cursorIcon);
  document.body.appendChild(cursor);

  let mouseX = window.innerWidth / 2;
  let mouseY = window.innerHeight / 2;
  let hasMoved = false;
  let animationFrame = 0;

  window.addEventListener('pointermove', (e) => {
    if (e.pointerType === 'touch') return;
    mouseX = e.clientX;
    mouseY = e.clientY;
    if (cursor.hidden) {
      cursor.hidden = false;
    }
    hasMoved = true;
    syncHoverState();
    if (!animationFrame) animationFrame = requestAnimationFrame(updateCursor);
  }, { passive: true });

  const interactiveSelector = [
    'a[href]',
    'button:not(:disabled)',
    '[role="button"]:not([aria-disabled="true"])',
    'input:not(:disabled)',
    'select:not(:disabled)',
    'textarea:not(:disabled)',
    '[tabindex]:not([tabindex="-1"])',
    '.scroll-indicator',
    '.landing-brand',
  ].join(',');

  document.addEventListener('pointerover', (event) => {
    if (event.pointerType === 'touch') return;
    syncHoverState(event.target);
  }, { passive: true });

  // Un clic puede abrir una capa nueva bajo un puntero inmóvil (el diccionario,
  // por ejemplo). Recalcular después del clic evita conservar el hover del
  // elemento que quedó detrás de esa capa.
  document.addEventListener('click', () => {
    queueMicrotask(() => syncHoverState());
  }, { passive: true });

  document.addEventListener('pointerdown', (event) => {
    if (event.pointerType !== 'touch') cursor.classList.add('is-pressed');
  }, { passive: true });
  document.addEventListener('pointerup', () => cursor.classList.remove('is-pressed'), { passive: true });
  document.addEventListener('pointercancel', () => cursor.classList.remove('is-pressed'), { passive: true });
  window.addEventListener('blur', hideCursor);
  document.addEventListener('visibilitychange', () => {
    if (document.hidden) hideCursor();
  });

  // Sincroniza la visibilidad al entrar o salir de la ventana.
  document.addEventListener('mouseleave', hideCursor);

  function hideCursor() {
    cursor.hidden = true;
    cursor.classList.remove('is-clickable', 'is-pressed');
  }

  document.addEventListener('mouseenter', () => {
    if (hasMoved) {
      cursor.hidden = false;
      syncHoverState();
    }
  });

  function syncHoverState(target = document.elementFromPoint(mouseX, mouseY)) {
    cursor.classList.toggle('is-clickable', Boolean(target?.closest?.(interactiveSelector)));
  }

  function updateCursor() {
    animationFrame = 0;

    cursor.style.transform = `translate3d(${mouseX}px, ${mouseY}px, 0)`;
  }
}
