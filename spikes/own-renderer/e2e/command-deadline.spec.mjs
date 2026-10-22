// ADR-0381: each send of a command is answered within its deadline, or it
// is no answer. A request held open with no answer, an answer that never
// begins and an answer whose body never ends were waited for without end
// (ADR-0173's "not claimed"): the press stayed pending, its line shown, and
// the page's order held each later press's command behind it (ADR-0305).
// Each send now has ten seconds for its whole answer, its body's last byte
// included. One that has none by then is aborted, and sent again as the
// command's `retry` clause allows, with its interaction, which the host
// answers with the first's outcome (ADR-0121). `?deadline=` lowers the ten
// seconds, so a test here waits a second and a half for a send.
//
// Each fault is the session's own, so the suite shares its host.
import { test } from "@playwright/test";
import { expect } from "../assertion-expect.mjs";

const ADD = "store.page.add_to_cart";
/** The deadline these tests give each send, in milliseconds. */
const DEADLINE = 1500;
/** A timer never fires early; the events that time it may each be late. */
const SLACK = 400;

/** A page, ready to be pressed. Polled: WebKit runs no animation frame
 * before its first paint (`slots.spec.mjs`). */
async function ready(page, path = "/stores/47") {
  await page.goto(path);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
}

const lines = (page) => page.locator("#cart-lines li");
/** A press that failed, as the runtime marks its control. */
const failed = (page) => page.locator("[data-pw-handler-error]");
const logged = (page, prefix) =>
  page.evaluate((p) => (window.__pw?.log ?? []).filter((l) => l.startsWith(p)).length, prefix);
/** How many presses have not finished (ADR-0280): none, once each is
 * settled, its answer read and its arms run. */
const pressing = (page) => page.evaluate(() => window.__pw.pressing());

/** Each request the page sends for `component`: when it was sent, and given
 * up or answered whole; its interaction; and the one it names as before it
 * (ADR-0305). */
function sent(page, component) {
  const requests = [];
  page.on("request", (r) => {
    if (new URL(r.url()).pathname !== `/command/${component}`) return;
    const headers = r.headers();
    requests.push({
      request: r,
      at: Date.now(),
      interaction: headers["pw-interaction"],
      after: headers["pw-after"] ?? null,
    });
  });
  const ended = (key) => (r) => {
    const entry = requests.find((x) => x.request === r);
    if (entry) entry[key] = Date.now();
  };
  page.on("requestfailed", ended("failed"));
  page.on("requestfinished", ended("finished"));
  return requests;
}
const given = (requests) => requests.filter((r) => r.failed !== undefined).length;
const finished = (requests) => requests.filter((r) => r.finished !== undefined).length;

/** No answer, and no failure either: the request is held open until the
 * page gives up on it. */
const neverAnswered = () => new Promise(() => {});

test("`?deadline=` and `?load-deadline=` each lower their own, never raise it, and never below half a second", async ({
  page,
}) => {
  const deadlines = async (query) => {
    await ready(page, `/stores/47${query}`);
    return page.evaluate(() =>
      typeof window.__pw.deadline === "function" && typeof window.__pw.loadDeadline === "function"
        ? `${window.__pw.deadline()} ${window.__pw.loadDeadline()}`
        : null,
    );
  };
  // Ten seconds each, unless a test asks for less.
  expect(await deadlines("")).toBe("10000 10000");
  expect(await deadlines(`?deadline=${DEADLINE}`)).toBe(`${DEADLINE} 10000`);
  expect(await deadlines(`?load-deadline=${DEADLINE}`)).toBe(`10000 ${DEADLINE}`);
  expect(await deadlines("?deadline=600000")).toBe("10000 10000");
  // Never below half a second: a link cannot make every press fail at once.
  expect(await deadlines("?deadline=1&load-deadline=1")).toBe("500 500");
  // What is no time above zero asks for nothing.
  expect(await deadlines("?deadline=0")).toBe("10000 10000");
  expect(await deadlines("?deadline=soon")).toBe("10000 10000");
});

test("a send no answer comes for is given up at its deadline and sent again, its line shown meanwhile", async ({
  page,
}) => {
  await ready(page, `/stores/47?deadline=${DEADLINE}`);
  const requests = sent(page, ADD);
  let calls = 0;
  await page.route(`**/command/${ADD}`, (route) =>
    ++calls === 1 ? neverAnswered() : route.continue(),
  );
  try {
    await page.getByRole("button", { name: "Add Espresso" }).click();
    // The press is shown while its request waits.
    await expect(lines(page)).toHaveCount(1);
    await expect(page.locator("#cart-count")).toHaveText("1");
    // Given up at its deadline, not before and not long after, then sent
    // again with its interaction.
    await expect.poll(() => given(requests), { timeout: 8_000 }).toBe(1);
    expect(requests[0].failed - requests[0].at).toBeGreaterThanOrEqual(DEADLINE - SLACK);
    expect(requests[0].failed - requests[0].at).toBeLessThan(DEADLINE + 2_000);
    await expect.poll(() => requests.length, { timeout: 8_000 }).toBe(2);
    expect(requests[1].interaction).toBe(requests[0].interaction);
    expect(await logged(page, `resent ${ADD}: no answer within ${DEADLINE} ms`)).toBe(1);
    // Answered whole, and the press settled: the server's.
    await expect.poll(() => finished(requests), { timeout: 8_000 }).toBe(1);
    await expect.poll(() => pressing(page), { timeout: 8_000 }).toBe(0);
    await expect(page.locator("#cart-count")).toHaveText("1");
    await expect(failed(page)).toHaveCount(0);
  } finally {
    await page.unrouteAll({ behavior: "ignoreErrors" });
  }
  // Read again: one line.
  await ready(page);
  await expect(page.locator("#cart-count")).toHaveText("1");
});

for (const [at, what] of [
  ["before", "held before it runs"],
  ["after", "held after it committed"],
  ["body", "held partway through its answer's body"],
]) {
  test(`a command whose connection is ${what} is sent again at its deadline, and is one line`, async ({
    page,
  }) => {
    await ready(page, `/stores/47?deadline=${DEADLINE}`);
    // The host holds this session's next command's connection open, with no
    // answer, or with an answer begun and never finished (ADR-0381's fault).
    expect((await page.request.post(`/bench/hold?next=command&at=${at}`)).ok()).toBe(true);
    const requests = sent(page, ADD);
    await page.getByRole("button", { name: "Add Espresso" }).click();
    await expect.poll(() => requests.length, { timeout: 8_000 }).toBe(2);
    expect(requests[1].interaction).toBe(requests[0].interaction);
    expect(await logged(page, `resent ${ADD}: no answer within ${DEADLINE} ms`)).toBe(1);
    // The resend answered whole, the held one never, and the press settled.
    await expect.poll(() => finished(requests), { timeout: 8_000 }).toBe(1);
    expect(requests[1].finished).not.toBe(undefined);
    await expect.poll(() => pressing(page), { timeout: 8_000 }).toBe(0);
    await expect(page.locator("#cart-count")).toHaveText("1");
    await expect(failed(page)).toHaveCount(0);
    // The server's: one line, though the command may have reached it twice.
    await ready(page);
    await expect(page.locator("#cart-count")).toHaveText("1");
  });
}

test("a press none of whose sends is answered fails at its last send's deadline, its line taken away", async ({
  page,
}) => {
  await ready(page, `/stores/47?deadline=${DEADLINE}`);
  const requests = sent(page, ADD);
  await page.route(`**/command/${ADD}`, neverAnswered);
  try {
    await page.getByRole("button", { name: "Add Espresso" }).click();
    await expect(failed(page)).toHaveCount(1, { timeout: 12_000 });
    // `retry transport_only(max = 2, jitter = true)`: the request and two
    // more, one interaction, each given up at its own deadline.
    expect(requests.length).toBe(3);
    expect(new Set(requests.map((r) => r.interaction)).size).toBe(1);
    await expect.poll(() => given(requests), { timeout: 4_000 }).toBe(3);
    for (const r of requests) expect(r.failed - r.at).toBeGreaterThanOrEqual(DEADLINE - SLACK);
    expect(await logged(page, `resent ${ADD}: no answer within ${DEADLINE} ms`)).toBe(2);
    await expect(failed(page)).toHaveAttribute("data-pw-handler-error", "unreachable");
    await expect(lines(page)).toHaveCount(0);
    await expect(page.locator("#cart-count")).toHaveText("0");
  } finally {
    await page.unrouteAll({ behavior: "ignoreErrors" });
  }
});

/** The cart page, its one Espresso's add answered before it is read. An
 * assertion with its own deadline, not a wait for a response: an add that is
 * never answered fails here, and holds no test past its own timeout. */
async function oneEspresso(page, path) {
  await ready(page);
  await page.getByRole("button", { name: "Add Espresso" }).click();
  await expect
    .poll(async () => (await logged(page, "reconciled cart")) > 0, { timeout: 10_000 })
    .toBe(true);
  await ready(page, path);
}

/** A deadline well inside the two seconds the host holds a command for one
 * not yet arrived (ADR-0305), so that a send held there is given up first. */
const HELD = 800;

test("a press made after one never answered runs once that one is given up", async ({ page }) => {
  await oneEspresso(page, `/cart?deadline=${HELD}`);
  // The order never reaches the server, and no answer comes for it.
  await page.route("**/command/store.page.place_order", neverAnswered);
  const orders = sent(page, "store.page.place_order");
  const clears = sent(page, "store.page.clear_cart");
  try {
    // From the keyboard: no pointer to miss a button the order's commit
    // moves (`navigate.spec.mjs`).
    await page.getByRole("button", { name: "Place order" }).press("Enter");
    await page.locator("#clear-cart").press("Enter");
    // The clear named the order, and was held for it; given up at its
    // deadline, it waited its turn, not its retry clause's; the order given
    // up, it is sent again naming none, and runs.
    const named = () =>
      clears
        .map((c) => (c.after === null ? "none" : c.after === orders[0]?.interaction ? "order" : "other"))
        .join(",");
    await expect.poll(named, { timeout: 15_000 }).toBe("order,none");
    expect(
      await logged(page, `store.page.clear_cart sent again: no answer within ${HELD} ms`),
    ).toBe(1);
    // Its turn: sent again only once the order's last send was given up.
    expect(orders.length).toBe(3);
    expect(clears[1].at - orders[2].at).toBeGreaterThanOrEqual(HELD - SLACK);
    await expect(page.locator("#cart-count")).toHaveText("0");
  } finally {
    await page.unrouteAll({ behavior: "ignoreErrors" });
  }
  // Read again: the clear ran, and no order was placed.
  await ready(page, "/cart");
  await expect(page.locator("#cart-count")).toHaveText("0");
  await ready(page, "/order");
  await expect(page.locator("#order-status")).toHaveText("You have no order yet.");
});

test("a send held for the one before it is no retry of its own", async ({ page }) => {
  await oneEspresso(page, `/cart?deadline=${HELD}`);
  // The order never reaches the server. The clear's first send does, and is
  // held there for the order; each send of it after that fails on its way.
  await page.route("**/command/store.page.place_order", neverAnswered);
  let routed = 0;
  await page.route("**/command/store.page.clear_cart", (route) =>
    ++routed === 1 ? route.continue() : route.abort("failed"),
  );
  const clears = sent(page, "store.page.clear_cart");
  try {
    await page.getByRole("button", { name: "Place order" }).press("Enter");
    await page.locator("#clear-cart").press("Enter");
    // Both fail: the order unanswered, the clear unreachable.
    await expect(failed(page)).toHaveCount(2, { timeout: 20_000 });
    // The held send, then the request and the two resends its clause
    // allows: being held cost the clear none of them.
    expect(clears.map((c) => (c.after === null ? "none" : "order")).join(",")).toBe(
      "order,none,none,none",
    );
  } finally {
    await page.unrouteAll({ behavior: "ignoreErrors" });
  }
});

test("a press made after one that declares no retry runs, though the host held it for that one", async ({
  page,
}) => {
  // `demo.pick`'s `pick` declares no `retry`: the first pick, never
  // answered, fails at its deadline; the second, named after it and held by
  // the host for it, is given up a moment later, after the first has failed.
  await ready(page, `/page/demo.pick.PickPage~ui?first=espresso&second=cortado&deadline=${HELD}`);
  const picks = sent(page, "demo.pick.pick");
  let calls = 0;
  await page.route("**/command/demo.pick.pick", (route) =>
    ++calls === 1 ? neverAnswered() : route.continue(),
  );
  try {
    await page.locator("#first button").click();
    await page.locator("#second button").click();
    // The second named the first, was held for it, and was sent again naming
    // none once the first had failed, and ran.
    const named = () =>
      picks
        .map((p) => (p.after === null ? "none" : p.after === picks[0]?.interaction ? "first" : "other"))
        .join(",");
    await expect.poll(named, { timeout: 10_000 }).toBe("none,first,none");
    await expect.poll(() => finished(picks), { timeout: 8_000 }).toBe(1);
    await expect.poll(() => pressing(page), { timeout: 8_000 }).toBe(0);
    // The first failed, unanswered; the second did not.
    await expect(failed(page)).toHaveCount(1);
    await expect(page.locator("#first button")).toHaveAttribute("data-pw-handler-error", "unreachable");
  } finally {
    await page.unrouteAll({ behavior: "ignoreErrors" });
  }
});

test("a command that declares no retry fails at its one send's deadline", async ({ page }) => {
  // `demo.pick`'s `pick` is idempotent and declares no `retry`.
  await ready(page, `/page/demo.pick.PickPage~ui?first=espresso&second=cortado&deadline=${DEADLINE}`);
  const requests = sent(page, "demo.pick.pick");
  await page.route("**/command/demo.pick.pick", neverAnswered);
  try {
    await page.locator("#first button").click();
    await expect(failed(page)).toHaveCount(1, { timeout: 6_000 });
    expect(requests.length).toBe(1);
    expect(await logged(page, "resent ")).toBe(0);
    await expect(failed(page)).toHaveAttribute("data-pw-handler-error", "unreachable");
  } finally {
    await page.unrouteAll({ behavior: "ignoreErrors" });
  }
});
