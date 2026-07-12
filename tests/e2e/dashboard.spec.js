const { test, expect } = require("@playwright/test");

test("dashboard carga sin errores ni logs de debug por defecto", async ({ page }) => {
  const errors = [];
  const logs = [];
  page.on("pageerror", error => errors.push(error.message));
  page.on("console", message => {
    if (message.type() === "error") errors.push(message.text());
    if (message.type() === "log") logs.push(message.text());
  });

  await page.goto("/");
  await expect(page.locator("#pnl")).toBeVisible();
  await expect(page.locator("#balances")).toBeAttached();
  await expect.poll(async () => page.locator("#balances .balance").count()).toBeGreaterThan(0);
  expect(errors).toEqual([]);
  expect(logs).toEqual([]);
});

test("salud, readiness y caching exponen contratos operativos", async ({ request }) => {
  const health = await request.get("/healthz");
  expect(health.ok()).toBeTruthy();
  expect(await health.json()).toMatchObject({ ok: true });
  expect(health.headers()["cache-control"]).toBe("no-store");

  const ready = await request.get("/readyz");
  expect([200, 503]).toContain(ready.status());
  const body = await ready.json();
  expect(typeof body.ready).toBe("boolean");
  expect(Array.isArray(body.checks)).toBeTruthy();

  const html = await request.get("/");
  expect(html.headers()["cache-control"]).toContain("no-cache");
  const asset = await request.get("/styles.css");
  expect(asset.headers()["cache-control"]).toContain("max-age=3600");
});

test("demo rentable mantiene PnL positivo y GA activo", async ({ request }) => {
  const response = await request.post("/api/demo", {
    data: { escenario: "mercado_rentable" },
  });
  expect(response.ok()).toBeTruthy();
  const state = await (await request.get("/api/estado")).json();
  expect(state.metricas.utilidadAcumuladaUsd).toBeGreaterThan(0);
  expect(state.operaciones.length).toBeGreaterThan(0);
  expect(state.genetico?.activo).toBeTruthy();
});
