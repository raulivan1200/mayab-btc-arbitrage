const gridContainer = document.getElementById("header-grid");

if (gridContainer) {
  let resizeTimer = 0;
  let tiles = [];
  let cols = 0;
  let rows = 0;
  let lastWidth = 0;
  let lastHeight = 0;
  let pointerFrame = 0;
  let pendingX = 0;
  let pendingY = 0;
  let hasPendingPointer = false;
  let lastPaintedIndex = -1;
  let cleanupTimer = 0;
  let nextCleanupAt = 0;
  const tileExpiry = new Map();
  const header = gridContainer.closest(".barra-superior");
  
  const colors = [
    "var(--blue)",
    "var(--green)",
    "var(--orange)",
    "var(--yellow)",
    "var(--purple)"
  ];
  
  function clearTileTimers() {
    window.clearTimeout(cleanupTimer);
    cleanupTimer = 0;
    nextCleanupAt = 0;
    tileExpiry.clear();
  }

  function createGrid() {
    clearTileTimers();
    gridContainer.innerHTML = "";
    tiles = [];

    const { width, height } = gridContainer.getBoundingClientRect();
    if (width < 1 || height < 1) return;
    lastWidth = Math.round(width);
    lastHeight = Math.round(height);
    const tileSize = width < 700 ? 34 : 44;
    cols = Math.max(1, Math.ceil(width / tileSize));
    rows = Math.max(1, Math.ceil(height / tileSize));
    
    gridContainer.style.gridTemplateColumns = `repeat(${cols}, 1fr)`;
    gridContainer.style.gridTemplateRows = `repeat(${rows}, 1fr)`;
    
    const numTiles = cols * rows;
    
    for (let i = 0; i < numTiles; i++) {
      const tile = document.createElement("div");
      tile.className = "grid-tile";
      
      const hoverColor = colors[Math.floor(Math.random() * colors.length)];
      tile.style.setProperty("--hover-c", hoverColor);
      
      gridContainer.appendChild(tile);
      tiles.push(tile);
    }
  }

  createGrid();

  window.addEventListener("resize", () => {
    window.clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(createGrid, 200);
  }, { passive: true });

  if ("ResizeObserver" in window) {
    const resizeObserver = new ResizeObserver(([entry]) => {
      const width = Math.round(entry.contentRect.width);
      const height = Math.round(entry.contentRect.height);
      if (width < 1 || height < 1 || (width === lastWidth && height === lastHeight)) return;
      window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(createGrid, 80);
    });
    resizeObserver.observe(gridContainer);
  }

  function cleanupTiles() {
    cleanupTimer = 0;
    nextCleanupAt = 0;
    const now = performance.now();
    let earliest = Infinity;

    tileExpiry.forEach((expiresAt, index) => {
      if (expiresAt <= now) {
        tiles[index]?.classList.remove("hovered");
        tileExpiry.delete(index);
      } else {
        earliest = Math.min(earliest, expiresAt);
      }
    });

    if (earliest < Infinity) scheduleCleanup(earliest);
  }

  function scheduleCleanup(expiresAt) {
    if (cleanupTimer && nextCleanupAt <= expiresAt) return;
    window.clearTimeout(cleanupTimer);
    nextCleanupAt = expiresAt;
    cleanupTimer = window.setTimeout(cleanupTiles, Math.max(0, expiresAt - performance.now()));
  }

  function paintTile(index, delay = 420) {
    const tile = tiles[index];
    if (!tile) return;

    tile.classList.add("hovered");
    const expiresAt = performance.now() + delay;
    tileExpiry.set(index, expiresAt);
    scheduleCleanup(expiresAt);
  }

  function paintPointer() {
    pointerFrame = 0;
    if (!hasPendingPointer) return;

    const rect = gridContainer.getBoundingClientRect();
    const col = Math.floor(((pendingX - rect.left) / rect.width) * cols);
    const row = Math.floor(((pendingY - rect.top) / rect.height) * rows);
    const index = row * cols + col;
    if (index === lastPaintedIndex) return;
    lastPaintedIndex = index;

    paintTile(index, 520);
    if (col > 0) paintTile(index - 1, 300);
    if (col < cols - 1) paintTile(index + 1, 300);
    paintTile(index - cols, 300);
    paintTile(index + cols, 300);
  }

  header?.addEventListener("pointermove", (event) => {
    if (event.pointerType === "touch") return;
    pendingX = event.clientX;
    pendingY = event.clientY;
    hasPendingPointer = true;
    if (!pointerFrame) pointerFrame = requestAnimationFrame(paintPointer);
  }, { passive: true });

  header?.addEventListener("pointerleave", () => {
    lastPaintedIndex = -1;
    hasPendingPointer = false;
  }, { passive: true });
}
