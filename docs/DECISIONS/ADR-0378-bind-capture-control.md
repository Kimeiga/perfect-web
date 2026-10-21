# ADR-0378: Binding capture controls exercise diagnostic ownership

Status: accepted by the integrator, 2026-10-20; E14; refines the witness for ADR-0142.

The combined `e14-bind` run passed eight baseline tests but killed only 13
of 14 controls. Removing the bound-handler capture exemption survived because
valid signal targets are separately excluded by canonical lexical signal
identity, while the existing invalid target was a serializable String.

The exemption remains meaningful. Binding a local function value is invalid
and already reports PW5304; its lowered value also independently has no text
form (PW0609). The generated binding handler must not additionally capture
that target and report PW5008. The new regression asserts the exact diagnostic
inventory, a valid signal neighbor, and an ordinary source-written handler
whose function capture must still report PW5008. No production semantics or
mutation anchor changes are needed. All 14 original fault models remain.

ADR-0142's one-diagnostic binding rule owns the invalid binding and its
synthetic handler. It does not suppress an independent lowered value's text
validation: this function-target fixture intentionally has both PW5304 and
PW0609. Only the additional synthetic capture diagnostic is incorrect.

The touched campaign previously credited builds without a test result as
kills. It now uses the shared Cargo native-exit/inventory parser and exact
native assertion source-line/column authority from the effect queue track,
with the shared process owner and source restoration guard. Baseline inventories
are frozen per command; build, runtime/setup, timeout, incomplete or memory
guard failures earn no kill. The shared helper and its four native regression
tests are copied byte-for-byte from effect source 0d616d04 so integration can
keep one authority.

The new witness passes, the original capture operator fails its native
assertion, and restored source passes. The four native producer regressions
pass. Raw evidence is recorded with this change. Full 14-control hosted
verification and exact-commit CI remain pending; no whole-campaign pass is
claimed from the single repaired control.
