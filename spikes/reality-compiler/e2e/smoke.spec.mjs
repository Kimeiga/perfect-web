import { expect, test } from "@playwright/test";

test("built app starts and deterministic controls work", async ({ page }) => {
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.goto("/");

  await expect(page).toHaveTitle("Reality Compiler");
  await expect(page.locator("#runtime")).toContainText(/WebGPU renderer|WebGL 2 fallback/);
  await expect(page.locator("#world")).toBeVisible();

  const canvasSize = await page.locator("#world").evaluate((canvas) => ({
    width: canvas.width,
    height: canvas.height,
  }));
  expect(canvasSize.width).toBeGreaterThan(0);
  expect(canvasSize.height).toBeGreaterThan(0);

  await page.getByRole("button", { name: "Void" }).click();
  await expect(page.locator("#status")).toHaveText("World program hot-swapped.");

  expect(pageErrors).toEqual([]);
});
