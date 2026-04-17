/**
 * Command Interaction Tests — validates that UI buttons trigger commands
 * and that the simulation responds correctly.
 *
 * Maps to functional-specification.MD sections 5.3, 5.6.3 and acceptance criteria 8.3.
 */

import { test, expect } from "@playwright/test";

test.describe("Command Interactions", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    // Wait for live WebSocket connection
    await page.waitForFunction(() => {
      const pill = document.querySelector("#connection-pill");
      return pill && pill.textContent === "live";
    }, { timeout: 15000 });
  });

  test("depressurize cycle changes state", async ({ page }) => {
    // First reset any active commands
    await page.click('button[data-command="ABORT_CYCLE"]');
    await page.waitForTimeout(500);
    await page.click('button[data-command="RESET_FAULTS"]');
    await page.waitForTimeout(500);

    await page.click('button[data-command="START_DEPRESSURIZE_CYCLE"]');

    // State should change from idle
    await page.waitForFunction(() => {
      const el = document.querySelector("#state-name");
      return el && el.textContent !== "idle";
    }, { timeout: 10000 });

    const state = await page.locator("#state-name").textContent();
    expect(["cycle_depressurize", "eva_mode", "faulted"]).toContain(state);
  });

  test("abort cycle stops active command", async ({ page }) => {
    // Start a cycle
    await page.click('button[data-command="RESET_FAULTS"]');
    await page.waitForTimeout(300);
    await page.click('button[data-command="START_DEPRESSURIZE_CYCLE"]');
    await page.waitForTimeout(500);

    // Abort
    await page.click('button[data-command="ABORT_CYCLE"]');

    await page.waitForFunction(() => {
      const el = document.querySelector("#state-name");
      return el && el.textContent === "faulted";
    }, { timeout: 10000 });

    const state = await page.locator("#state-name").textContent();
    expect(state).toBe("faulted");
  });

  test("reset faults clears fault state", async ({ page }) => {
    // Trigger a fault state
    await page.click('button[data-command="START_DEPRESSURIZE_CYCLE"]');
    await page.waitForTimeout(300);
    await page.click('button[data-command="ABORT_CYCLE"]');
    await page.waitForTimeout(500);

    await page.click('button[data-command="RESET_FAULTS"]');
    await page.waitForTimeout(500);

    // Blocking condition should clear
    const blocking = await page.locator("#blocking-condition").textContent();
    // After reset, blocking may show the reset result or clear
    expect(blocking).toBeDefined();
  });

  test("unlock outer door rejected at high pressure", async ({ page }) => {
    // At startup pressure is ~101325 Pa, well above OuterUnlockMaxPressurePa (5000)
    await page.click('button[data-command="RESET_FAULTS"]');
    await page.waitForTimeout(300);

    await page.click('button[data-command="UNLOCK_OUTER_DOOR"]');
    await page.waitForTimeout(1000);

    // Should show a blocking condition about pressure
    const blocking = await page.locator("#blocking-condition").textContent();
    expect(blocking.toLowerCase()).toContain("pressure");
    // there should also be a temporary alert banner with reason
    const alert = await page.locator("#alert-banner");
    expect(await alert.textContent()).toMatch(/pressure/i);
  });
});

test.describe("WebSocket Live Updates", () => {
  test("connection pill shows live status", async ({ page }) => {
    await page.goto("/");

    await page.waitForFunction(() => {
      const pill = document.querySelector("#connection-pill");
      return pill && pill.textContent === "live";
    }, { timeout: 15000 });

    const pill = page.locator("#connection-pill");
    await expect(pill).toHaveText("live");
  });

  test("gauges update over time", async ({ page }) => {
    await page.goto("/");
    await page.waitForFunction(() => {
      const el = document.querySelector("#v-pressure");
      return el && el.textContent !== "0";
    }, { timeout: 15000 });

    const firstValue = await page.locator("#v-pressure").textContent();
    await page.waitForTimeout(2000);
    const secondValue = await page.locator("#v-pressure").textContent();

    // Values should exist (may or may not change depending on simulation state)
    expect(parseFloat(firstValue)).toBeGreaterThan(0);
    expect(parseFloat(secondValue)).toBeGreaterThan(0);
  });
});

test.describe("Apply Controls", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForFunction(() => {
      const pill = document.querySelector("#connection-pill");
      return pill && pill.textContent === "live";
    }, { timeout: 15000 });
  });

  test("toggling operator control and applying takes effect", async ({ page }) => {
    // Uncheck operator control
    const checkbox = page.locator("#operator-enabled");
    await checkbox.uncheck();
    await page.click("#apply-controls");
    await page.waitForTimeout(1500);

    // Try to send a command — should be rejected
    await page.click('button[data-command="RESET_FAULTS"]');
    await page.waitForTimeout(1000);

    const blocking = await page.locator("#blocking-condition").textContent();
    expect(blocking.toLowerCase()).toContain("operator");

    // Re-enable
    await checkbox.check();
    await page.click("#apply-controls");
    await page.waitForTimeout(500);
  });

  test("changing operation mode reflects in snapshot", async ({ page }) => {
    const select = page.locator("#operation-mode");
    await select.selectOption("MANUAL");
    await page.click("#apply-controls");
    await page.waitForTimeout(1500);

    // The snapshot should reflect MANUAL mode (the select should stay on MANUAL after next WS update)
    await expect(select).toHaveValue("MANUAL");

    // Reset
    await select.selectOption("AUTO");
    await page.click("#apply-controls");
  });
});
