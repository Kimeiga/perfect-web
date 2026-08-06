/* Host implementations for the resources generated templates import.
 *
 * ADR-0018: the compiler emits a manifest saying HOW a resource behaves; the
 * host decides WHAT it does. This file is that decision for the spike, and it
 * is hand-written on purpose — nothing generates the fetch yet.
 *
 * The delay models charter §15.5's recommendation latency, so the streaming
 * measurement has something real to wait for.
 */
const RECOMMENDATION_DELAY_MS = 1200;

export function Recommendations(_id) {
  return new Promise((resolve) =>
    setTimeout(
      () => resolve([{ id: "a", name: "Affogato" }, { id: "g", name: "Gibraltar" }]),
      RECOMMENDATION_DELAY_MS,
    ),
  );
}
