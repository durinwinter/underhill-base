/**
 * UI Panel Tests — validates all three required interfaces exist and update live.
 *
 * Maps to functional-specification.MD sections 5.6 and acceptance criteria 8.3.
 */

import { test, expect } from "@playwright/test";

test.describe("Panel A: OPC UA Connectivity & Diagnostics", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    // Wait for initial snapshot to populate
    await page.waitForFunction(() => {
      const el = document.querySelector("#endpoint");
      return el && el.textContent !== "-";
    }, { timeout: 15000 });
  });

  test("displays endpoint URL", async ({ page }) => {
    const endpoint = page.locator("#endpoint");
    await expect(endpoint).toContainText("mars-airlock");
  });

  test("displays active security mode", async ({ page }) => {
    const security = page.locator("#security");
    await expect(security).not.toHaveText("-");
  });

  test("displays client count", async ({ page }) => {
    const clients = page.locator("#clients");
    // With our WS connection, count should be >= 1
    await expect(clients).not.toHaveText("-");
  });

  test("displays publishing rate", async ({ page }) => {
    const rate = page.locator("#pub-rate");
    await expect(rate).not.toHaveText("0");
  });

  test("shows connected sessions section", async ({ page }) => {
    const sessionList = page.locator("#session-list");
    await expect(sessionList).toBeVisible();
  });

  test("shows MTP modes controls", async ({ page }) => {
    await expect(page.locator("#operator-enabled")).toBeVisible();
    await expect(page.locator("#remote-enabled")).toBeVisible();
    await expect(page.locator("#command-en")).toBeVisible();
    await expect(page.locator("#apply-en")).toBeVisible();
    const select = page.locator("#security-select");
    await select.selectOption("BASIC256SHA256");

    // Wait for the value to be reflected back from backend
    await page.waitForTimeout(1500);
    const security = page.locator("#security");
    await expect(security).toHaveText("BASIC256SHA256");

    // Reset
    await select.selectOption("NONE");
  });
});

test.describe("Panel B: 3D Real-Time Airlock View", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForFunction(() => {
      const el = document.querySelector("#state-name");
      return el && el.textContent !== "-";
    }, { timeout: 15000 });
  });

  test("3D view area or fallback canvas exists", async ({ page }) => {
    const threeWrap = page.locator("#three-wrap");
    await expect(threeWrap).toBeVisible();

    // Either a Three.js canvas or fallback canvas should be present
    const canvas = threeWrap.locator("canvas");
    await expect(canvas).toBeVisible();
  });

  test("status strip shows state name", async ({ page }) => {
    const stateName = page.locator("#state-name");
    await expect(stateName).not.toHaveText("-");
  });

  test("status strip shows active command info", async ({ page }) => {
    await expect(page.locator("#active-command")).toBeVisible();
    await expect(page.locator("#active-source")).toBeVisible();
    await expect(page.locator("#active-progress")).toBeVisible();
  });
});

test.describe("Panel C: Fendt-Themed HMI + P&ID", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForFunction(() => {
      const el = document.querySelector("#v-pressure");
      return el && el.textContent !== "0";
    }, { timeout: 15000 });
  });

  test("pressure gauge shows nonzero value", async ({ page }) => {
    const pressure = page.locator("#v-pressure");
    const text = await pressure.textContent();
    const value = parseFloat(text);
    expect(value).toBeGreaterThan(0);
  });

  test("temperature gauge shows value in range", async ({ page }) => {
    const temp = page.locator("#v-temp");
    const text = await temp.textContent();
    const value = parseFloat(text);
    expect(value).toBeGreaterThan(100);
    expect(value).toBeLessThan(400);
  });

  test("O2 gauge shows value", async ({ page }) => {
    const o2 = page.locator("#v-o2");
    const text = await o2.textContent();
    const value = parseFloat(text);
    expect(value).toBeGreaterThan(0);
  });

  test("command buttons are present", async ({ page }) => {
    await expect(page.locator('button[data-command="START_DEPRESSURIZE_CYCLE"]')).toBeVisible();
    await expect(page.locator('button[data-command="START_PRESSURIZE_CYCLE"]')).toBeVisible();
    await expect(page.locator('button[data-command="ABORT_CYCLE"]')).toBeVisible();
    await expect(page.locator('button[data-command="RESET_FAULTS"]')).toBeVisible();
    await expect(page.locator('button[data-command="UNLOCK_INNER_DOOR"]')).toBeVisible();
    await expect(page.locator('button[data-command="UNLOCK_OUTER_DOOR"]')).toBeVisible();
  });

  test("operator and fault panels are visible", async ({ page }) => {
    await expect(page.locator(".hmi-control-panel h3", { hasText: "Operator Commands" })).toBeVisible();
    await expect(page.locator(".hmi-control-panel h3", { hasText: "Fault Injection" })).toBeVisible();
    await expect(page.locator("#leak-rate")).toBeVisible();
    await expect(page.locator("#fault-apply-leak")).toBeVisible();
    await expect(page.locator("#fault-reset-leak")).toBeVisible();
  });

  test("fault injection presets update leak rate display", async ({ page }) => {
    await page.locator('button[data-fault-rate="0.0080"]').click();
    await expect(page.locator("#fault-current-leak")).toContainText("0.0080");

    await page.locator("#fault-reset-leak").click();
    await expect(page.locator("#fault-current-leak")).toContainText("0.0005");
  });

  test("P&ID SVG is rendered", async ({ page }) => {
    const svg = page.locator(".pid-svg");
    await expect(svg).toBeVisible();

    // Check key P&ID elements
    await expect(page.locator("#habitat-box")).toBeVisible();
    await expect(page.locator("#chamber-box")).toBeVisible();
    await expect(page.locator("#mars-box")).toBeVisible();
    await expect(page.locator("#equalize-valve")).toBeVisible();
    await expect(page.locator("#vent-valve")).toBeVisible();
    await expect(page.locator("#pump")).toBeVisible();
  });

  test("door state text updates", async ({ page }) => {
    const doorState = page.locator("#door-state");
    await expect(doorState).toContainText("Doors:");
  });

  test("valve state text updates", async ({ page }) => {
    const valveState = page.locator("#valve-state");
    await expect(valveState).toContainText("Valves:");
  });

  test("alarm state text shows", async ({ page }) => {
    const alarmState = page.locator("#alarm-state");
    await expect(alarmState).toContainText("Alarms:");
  });

  test("event log section exists", async ({ page }) => {
    const eventLog = page.locator("#event-log");
    await expect(eventLog).toBeVisible();
  });
});
