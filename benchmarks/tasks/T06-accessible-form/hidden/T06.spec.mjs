// T06's hidden tests: a request form that works for keyboard and screen
// reader users. Read from labels, roles, ARIA state and the accessible
// description, never from a stack's markup: a `role="alert"` and an
// `aria-live` region announce alike.

import { test, expect } from "./fixtures.mjs";

const field = (page) => page.getByLabel("Item to request", { exact: true });

/** Every mutation request the page sends from now on. */
function mutations(page) {
  const sent = [];
  page.on("request", (r) => {
    if (r.method() === "POST") sent.push(r.url());
  });
  return sent;
}

/** Is the text inside something a screen reader announces when it changes? */
const announced = (locator) =>
  locator.evaluate(
    (el) =>
      el.closest('[role="alert"], [role="status"], [aria-live="polite"], [aria-live="assertive"]') !==
      null,
  );

test("the field has a visible label tied to it", async ({ page, store }) => {
  await store.open();
  await expect(field(page)).toBeVisible();
  await expect(field(page)).toHaveRole("textbox");
  await expect(page.getByText("Item to request", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Request" })).toBeVisible();
});

test("an empty request shows an error tied to the field, and announced", async ({
  page,
  store,
}) => {
  await store.open();
  const sent = mutations(page);
  await page.getByRole("button", { name: "Request" }).click();
  const error = page.getByText("Enter an item to request.", { exact: true });
  await expect(error).toBeVisible();
  await expect(field(page)).toHaveAttribute("aria-invalid", "true");
  await expect(field(page)).toHaveAccessibleDescription("Enter an item to request.");
  expect(await announced(error), "the error is announced").toBe(true);
  expect(sent, "nothing is sent").toEqual([]);
});

test("Enter submits; the thanks is announced and the field empties", async ({
  page,
  store,
}) => {
  await store.open();
  const sent = mutations(page);
  await field(page).fill("Mocha");
  await field(page).press("Enter");
  const thanks = page.getByText("Thanks! We'll ask the store about Mocha.", { exact: true });
  await expect(thanks).toBeVisible();
  expect(await announced(thanks), "the thanks is announced").toBe(true);
  await expect(field(page)).toHaveValue("");
  expect(sent, "nothing is sent").toEqual([]);
  // The page is the one it was: Enter did not submit it to the server.
  await expect(store.count()).toHaveText("0");
});

test("a request after an error clears the error", async ({ page, store }) => {
  await store.open();
  await page.getByRole("button", { name: "Request" }).click();
  await expect(field(page)).toHaveAttribute("aria-invalid", "true");
  await field(page).fill("Mocha");
  await page.getByRole("button", { name: "Request" }).click();
  await expect(page.getByText("Thanks! We'll ask the store about Mocha.")).toBeVisible();
  await expect(page.getByText("Enter an item to request.")).toHaveCount(0);
  await expect(field(page)).not.toHaveAttribute("aria-invalid", "true");
  await expect(field(page)).not.toHaveAccessibleDescription("Enter an item to request.");
});

test("what is typed is shown as text", async ({ page, store }) => {
  await store.open();
  await field(page).fill("<b>x</b> & y");
  await page.getByRole("button", { name: "Request" }).click();
  await expect(
    page.getByText("Thanks! We'll ask the store about <b>x</b> & y.", { exact: true }),
  ).toBeVisible();
  await expect(page.locator("main b")).toHaveCount(0);
});
