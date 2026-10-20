// TodoMVC's own spec checks that an app saved its todos with
// `checkItemSaved`, which asks a spy on `Storage.prototype.setItem` whether
// it was called and then resets it: `cy.wrap(spy).invoke('reset')`
// (spikes/todomvc-cypress/cypress/e2e/spec.cy.js). Each test here passes
// where what it names holds in this Cypress, with no app at all.

it("a spy has resetHistory, and no reset", () => {
  const o = { m() {} };
  const spy = cy.spy(o, "m");
  o.m();
  console.log(
    `PROBE cypress ${Cypress.version}: reset:${typeof spy.reset} resetHistory:${typeof spy.resetHistory}`,
  );
  expect(typeof spy.resetHistory, "resetHistory").to.equal("function");
  expect(typeof spy.reset, "reset").to.equal("undefined");
});

it("the spec's own call, invoke('reset'), errs as its tests do", () => {
  cy.on("fail", (err) => {
    // What tests 4 and 9 of the run print.
    if (!err.message.includes("the property: `reset` does not exist on your subject")) throw err;
  });
  const o = { m() {} };
  const spy = cy.spy(o, "m");
  o.m();
  cy.wrap(spy, { log: false })
    .invoke({ timeout: 500 }, "reset")
    .then(() => {
      throw new Error("reset was invoked");
    });
});
