# Master failure taxonomy and census

**Audited repository revision:** `0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45`. **Research date:** 2026-09-10.

This file supplies **Part 2** (hierarchical taxonomy) and **Part 3** (failure census). Group headings are parent categories; each row is one invariant-level leaf. Cross-cutting AI, performance, interop, and security views link these leaves rather than duplicate them.

**Status semantics:** `covered` means a narrowly scoped implementation with recorded evidence was inspected, not rerun or proved universally. `partially covered` means mechanisms/witnesses exist but the stated broader invariant remains open. `specified but unproven` means an intended contract exists without sufficient inspected implementation evidence. `missing` means no sufficiently precise contract for the leaf was found in the inspected design; it is not proof that the term never appears elsewhere. `intentionally outside scope` marks residual guarantees the platform cannot own.

**Layers:** A construction; B static check; C compiler derivation; D runtime; E host/deployment; F browser/platform; G deterministic test/model check; H domain policy; I outside available knowledge. Combined layers are deliberate: a static admission rule does not replace a runtime security boundary. P0/P1 markers flag particularly urgent findings; the canonical priority plan is in `REQUIREMENTS.md`.

**DX profiles:** T value/type boundary; E inferred effects/authority; R owned runtime protocol; U native UI; X foreign/host boundary; D explicit domain policy; P performance planning; V verification/tooling. The complete cost/soundness review for these profiles is in `README.md`. A profile is a design review obligation, not a claim that a rule has zero cost.

**Evidence:** S identifiers resolve to checked primary sources; R identifiers to commit-pinned repository evidence. Model/spec examples are constructed failure scenarios, not historical incidents. Existing systems listed identify a technique or scoped mechanism, not proof that any whole framework eliminates the failure.

## Census chapters

The six chapters below are the authoritative classification tables. They are assembled in this order by the validator; the JSON export is derived.

- [A–D: Value meaning and language soundness through Principals, sessions, and authorization over time](taxonomy/01-language-lifecycle.md)
- [E–H: Privacy, integrity, and data lifecycle through Database invariants and query semantics](taxonomy/02-security-privacy.md)
- [I–L: Replication, ordering, and distributed policy through DOM identity, rendering, and resumption](taxonomy/03-data-distribution-rendering.md)
- [M–P: Forms, drafts, routes, and navigation through CSS, layout, and rendering cost](taxonomy/04-forms-accessibility-browser.md)
- [Q–T: Page lifecycle, local storage, and offline operation through Compatibility, rollout, and long-lived state](taxonomy/05-media-evolution-supply-chain.md)
- [U–X: Observability, tests, and evidence integrity through Operations, recoverability, and limits of automation](taxonomy/06-verification-operations-ai.md)

[S01]: SOURCES.md#s01
[S02]: SOURCES.md#s02
[S03]: SOURCES.md#s03
[S04]: SOURCES.md#s04
[S05]: SOURCES.md#s05
[S06]: SOURCES.md#s06
[S07]: SOURCES.md#s07
[S08]: SOURCES.md#s08
[S09]: SOURCES.md#s09
[S10]: SOURCES.md#s10
[S11]: SOURCES.md#s11
[S12]: SOURCES.md#s12
[S13]: SOURCES.md#s13
[S14]: SOURCES.md#s14
[S15]: SOURCES.md#s15
[S16]: SOURCES.md#s16
[S17]: SOURCES.md#s17
[S18]: SOURCES.md#s18
[S19]: SOURCES.md#s19
[S20]: SOURCES.md#s20
[S21]: SOURCES.md#s21
[S22]: SOURCES.md#s22
[S23]: SOURCES.md#s23
[S24]: SOURCES.md#s24
[S25]: SOURCES.md#s25
[S26]: SOURCES.md#s26
[S27]: SOURCES.md#s27
[S28]: SOURCES.md#s28
[S29]: SOURCES.md#s29
[S30]: SOURCES.md#s30
[S31]: SOURCES.md#s31
[S32]: SOURCES.md#s32
[S33]: SOURCES.md#s33
[S34]: SOURCES.md#s34
[S35]: SOURCES.md#s35
[S36]: SOURCES.md#s36
[S37]: SOURCES.md#s37
[S38]: SOURCES.md#s38
[S39]: SOURCES.md#s39
[S40]: SOURCES.md#s40
[S41]: SOURCES.md#s41
[S42]: SOURCES.md#s42
[S43]: SOURCES.md#s43
[S44]: SOURCES.md#s44
[S45]: SOURCES.md#s45
[S46]: SOURCES.md#s46
[S47]: SOURCES.md#s47
[S48]: SOURCES.md#s48
[S49]: SOURCES.md#s49
[S50]: SOURCES.md#s50
[S51]: SOURCES.md#s51
[S52]: SOURCES.md#s52
[S53]: SOURCES.md#s53
[S54]: SOURCES.md#s54
[S55]: SOURCES.md#s55
[S56]: SOURCES.md#s56
[S57]: SOURCES.md#s57
[S58]: SOURCES.md#s58
[S59]: SOURCES.md#s59
[S60]: SOURCES.md#s60
[S61]: SOURCES.md#s61
[S62]: SOURCES.md#s62
[S63]: SOURCES.md#s63
[S64]: SOURCES.md#s64
[S65]: SOURCES.md#s65
[S66]: SOURCES.md#s66
[S67]: SOURCES.md#s67
[S68]: SOURCES.md#s68
[S69]: SOURCES.md#s69
[S70]: SOURCES.md#s70
[S71]: SOURCES.md#s71
[S72]: SOURCES.md#s72
[S73]: SOURCES.md#s73
[S74]: SOURCES.md#s74
[S75]: SOURCES.md#s75
[S76]: SOURCES.md#s76
[R1]: SOURCES.md#r1
[R2]: SOURCES.md#r2
[R3]: SOURCES.md#r3
[R4]: SOURCES.md#r4
[R5]: SOURCES.md#r5
[R6]: SOURCES.md#r6
[R7]: SOURCES.md#r7
[R8]: SOURCES.md#r8
[R9]: SOURCES.md#r9
[R10]: SOURCES.md#r10
[R11]: SOURCES.md#r11
[R12]: SOURCES.md#r12
[R13]: SOURCES.md#r13
[R14]: SOURCES.md#r14
[R15]: SOURCES.md#r15
