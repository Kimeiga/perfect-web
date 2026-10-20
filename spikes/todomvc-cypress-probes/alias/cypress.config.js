// An alias read again after a reload, in the Cypress `spikes/todomvc-cypress`
// pins (track `device-store`, ADR-0366), over a static page with no app
// (`site/index.html`), served on PROBE_PORT by the probes' run.sh.
export default {
  e2e: {
    supportFile: false,
    specPattern: "cypress/e2e/*.cy.js",
    video: false,
    baseUrl: `http://127.0.0.1:${process.env.PROBE_PORT ?? 15499}`,
  },
};
