# ADR-0267: a component's trap says why it stopped

Status: accepted under the owner's delegation of 2026-10-02; queued by
ADR-0259's "Not claimed", for ruling 0057-c's "by name" (ADR-0210).
Date: 2026-10-08. Milestone: E14.

## Context

- **Every trap in a component read alike.** The backend stops an
  invocation with Wasm's `unreachable`, and Wasmtime's error said "wasm
  `unreachable` instruction executed", whatever stopped it: an `Int` that
  overflowed, a map from outside that repeats a key, memory that would not
  grow. The host reported a failed call (KNOWN_LIMITATIONS, "Traps are not
  distinguished by cause").
- **The browser's module names each**, `Error("trap: <cause>")` (ADR-0259
  and before it), so ruling 0057-c's "a repeated key still stops the
  invocation, by name" held in the browser and not on the server.
- **The component's own traps have seven causes**: an `Int` overflow (`+`,
  `-`, `*`, negation), a map or set from outside that repeats a key
  (ADR-0259), `Map.from_lists` given lists of two lengths, a code point not
  a Unicode scalar value (`String.from_codepoints`), a value too long for
  memory (a length past `i32::MAX` bytes: a concatenation, an array, a
  join, a graph's encoding), memory exhausted (`cabi_realloc` past the
  address space, or `memory.grow` refused), and a value from outside whose
  nodes are no tree (ADR-0194's decoding). **Wasm's own** stop it too: a
  zero divisor and the least `Int` divided by -1 (`i64.div_s`, which `/`
  and the multiplication check use), fuel spent, and the stack exhausted.
- **Wasmtime 48.0.5** (its source, read 2026-10-08): a trap's error
  carries a `WasmBacktrace`, its `frames()` the youngest first, and
  `FrameInfo::func_name()` the function's name from the module's name
  section. Its own documented example names a function `$trap` whose body
  is `unreachable` and reads `frames[0].func_name() == Some("trap")`. The
  method is "primarily used for debugging and human-readable purposes", its
  "exact return value may be tweaked over time". A backtrace is captured by
  default, at most 20 frames (`Config::wasm_backtrace_max_frames`), and
  "backtraces may omit inlined stack frames"; inlining is off by default
  (`Config::compiler_inlining`, `Inlining::No`).
- **The alternatives**, each weighed:
  - an operation the component imports and calls with its cause: robust
    against a name's change, but every component, a pure one too, would
    import from its host, and every host, the browser's included, provide
    it. A component's imports are its capabilities, audited
    (`imports_of`), and a trap is none;
  - the cause written to memory or a global before the trap, for the host
    to read: a trapped instance is not entered again, and a component
    exports no core memory or global to its host;
  - a custom section mapping each trap's code offset to its cause: as
    robust as a name and more machinery, a format of the project's own
    beside the name section every Wasm tool already reads.

## Decision

1. **Each cause is a function**, `[] -> []`, its body `unreachable`, and
   each trap of the component's own calls its cause's function before its
   own `unreachable`. The trap stops where it did, whatever the call does,
   and the module validates as before.
2. **The name section names each `pw-trap: <its words>`**: the browser's
   module's words where both have the cause ("Int overflow", "a map or set
   from outside repeats a key", "lists of two lengths", "not a Unicode
   scalar value"), and the component's own for its own ("a value too long
   for memory", "memory exhausted", "a value from outside whose nodes are
   no tree").
3. **The host says why a call stopped**: `stopped: <cause>: <Wasmtime's
   error>`, the cause read from the first frame of the trap's backtrace
   whose name has the prefix, or else Wasm's own by its code: "division by
   zero", "Int overflow", "out of fuel", "calls nested too deep". Any other
   error reads as Wasmtime's, whole, as before.
4. **The host's engine states what reading a frame needs**: a backtrace
   captured, at most 20 frames, the youngest first, so the trap's own
   frame is in it; and no function inlined into its caller. Both are
   Wasmtime 48's defaults, stated in `engine_config` (ADR-0244) so that a
   release changing them does not drop the causes silently. Each is load
   bearing: with `Inlining::Yes` the cause functions are inlined, their
   frames are not in the backtrace, and the causes are lost, as the mutant
   that inlines shows.
5. **The prefix is one constant each side**, `pw_core::backend::wasm::
   TRAP_PREFIX` and `pw_host::engine::TRAP_PREFIX`: the host depends on no
   compiler crate, and a test holds them equal.

## Acceptance

- **`compiler/pw-conformance/tests/trap_causes.rs`**, through the host:
  each cause of the component's own, an `Int` overflow by `+`, `*` and
  negation, a repeated key, lists of two lengths, a code point not a scalar
  value, a value too long for memory (a join of 2048 copies of one MiB,
  2 GiB, refused before anything is allocated), and memory exhausted (a
  join of 1024, 1 GiB, past the 64 MiB the harness gives a call, and a
  string doubled past it), each with its control; Wasm's own, a zero
  divisor, the least `Int` divided by -1, fuel spent, and calls nested too
  deep; and the two prefixes equal.
- **`compiler/pw-conformance/tests/recursive_types.rs`**: each of ten
  malformed node lists stops "a value from outside whose nodes are no
  tree".
- **`compiler/pw-conformance/tests/javascript.rs`**: the browser's module
  and the component agree on why a call stopped, where they agreed only
  that both did.
- **`scripts/trap_cause_mutations.py`, 16 mutants**: no function named by
  its cause; the causes' functions looked for one place on; each tested
  cause's site calling another cause's function (an overflow, a repeated
  key, lists of two lengths, a code point, memory that does not grow, a
  join too long, nodes that are no tree); the host reading no cause from
  the frame, and not naming a zero divisor, the least `Int` divided by -1,
  fuel spent, or calls nested too deep; and the host's engine capturing no
  backtrace, or inlining. Recorded by `just e14-trap-causes`.
- **The scripts whose mutated code ADR-0267 changed, each run whole**, every
  mutant killed: `pure` (16; its four overflow mutants re-anchored to the
  helpers' new signatures), `function_value` (7), `boxed_types` (17),
  `recursive_types` (16), `case` (9), `slice` (10), `stdlib` (16),
  `arrival` (3), `map` (15), `operand` (8), `descriptions` (8) and
  `invariants` (25). The last's first baseline was red, and found the
  committed components stale (below).
- **The scripts whose only change is the browser test's**: not run again.
  It compares a trap by its cause where it compared `true`, so every
  comparison that differed before still differs.
- **The whole workspace's tests**, the corpus among them; and the
  committed components compiled again, the store's (`just e10-component`)
  and kiokun's (`just e10-kiokun`, its host's tests and its browser suite
  in three engines passing on them): each holds the seven functions and the
  name section now, 315 bytes more.

## Not claimed

- **A cause through a tool that strips names**: `wasm-tools strip` or an
  optimizer's debug stripping removes the name section, and with it the
  causes; the traps stay where they were, and read as Wasm's own. Nothing
  in the build strips it.
- **A cause where an engine runs out**: the browser's engine has its own
  limits, a string's length among them, and its words for them. The two
  agree on the program's causes, not on its engine's.
- **What a page shows of a failed call** is unchanged: the cause is the
  host's to report, in its errors, logs and evidence. No cause reaches the
  browser.
- **A trap Wasm raises that the backend should never reach** (an access
  out of bounds, an indirect call's type) reads as Wasmtime's own message:
  it would be the backend's defect, not a cause.
