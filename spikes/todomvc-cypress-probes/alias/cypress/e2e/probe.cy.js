// TodoMVC's own persistence test aliases each todo its custom command
// `createTodo` found, a command that ends in `.then()`
// (spikes/todomvc-cypress/cypress/support/commands.js), reloads the page, and
// reads each alias again (spikes/todomvc-cypress/cypress/e2e/spec.cy.js,
// "should persist its data"). Each test here passes where what it names
// holds in this Cypress, over a static page with no app and no storage.

// The spec's command, its lookup as it is written, logging left out.
Cypress.Commands.add("findTodo", function (todo) {
  cy.get(".todo-list", { log: false })
    .contains("li", todo, { log: false })
    .then(function () {});
});

it("an alias a custom command made with .then() is not read again after a reload", () => {
  cy.on("fail", (err) => {
    // What test 24 of the run prints.
    if (!err.message.includes("Cypress cannot requery the page")) throw err;
  });
  cy.visit("/");
  cy.findTodo("one").as("first");
  cy.get("@first").should("contain", "one");
  cy.reload();
  cy.get("@first", { timeout: 1000 })
    .should("contain", "one")
    .then(() => {
      throw new Error("the alias was read again");
    });
});

it("the control: an alias cy.get made is read again after a reload", () => {
  cy.visit("/");
  cy.get(".todo-list li").first().as("first");
  cy.get("@first").should("contain", "one");
  cy.reload();
  cy.get("@first").should("contain", "one");
});
