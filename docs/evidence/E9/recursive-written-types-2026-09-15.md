# Recursive written-type repair, 2026-09-15

## Provenance and scope

Base master: `0f2a4cf58b4e68688cc2fda876cc74b9b1c2b9b7`.
Local source snapshot: `5b10b25d3c6b7afc7054c585c6fb02aec0033d11`, the base
plus a temporary, read-only toolchain workflow. That workflow is removed from
the final tree. Decision: [ADR-0028](../../DECISIONS/ADR-0028-recursive-written-types-before-signature-cutover.md).

Real Rust 1.97.1 (`8bab26f4f`, LLVM 22.1.6) and locked Cargo registry artifacts
were obtained through GitHub Actions run 35043990036. The downloaded archive's
SHA-256 was verified as
`2c559ccd95631496ac624986628dcb372fdeeb6a173d9afbec4f62abb06ee7a2`, and each
inner archive was independently checked. Only the toolchain's bin/lib and Cargo
registry were copied, not credential/config files. Local checks used the actual
compiler and runtime code, with network access disabled.

## Regressions observed before changing implementation

The first five tests in `compiler/pw-core/tests/recursive_declared_types.rs` all
failed against the base compiler after successfully parsing their source:

| Test | Before |
|---|---|
| Nested Result/List/Option | Resolver reported nested generics unsupported |
| Unknown nested leaf | The outer unsupported result hid the unresolved leaf |
| Built-in constructor arity | Nonempty but incorrect argument lists resolved |
| Qualified function used as a type | A term declaration became a nominal type identity |
| Unit `()` | Grammar accepted it, resolver did not recognize it |

The repaired resolver passes those cases. Six additional tests establish
parameter/field/return structure, complete serialized resource manifests,
nominal differences, argument order, stable identities after source reordering,
namespace/visibility boundaries, and diagnostic provenance. A direct generic
resource manifest previously kept only its constructor; it now preserves the
whole type. Tests distinguish absent return annotations from specified types.

## Executed checks

| Local check | Observed result |
|---|---|
| `cargo test --locked -p pw-core --lib` | 178 passed |
| All 51 `pw-core` integration targets, in batches | 399 passed, 0 failed |
| `cargo clippy --locked -p pw-core --all-targets -- -D warnings` | Exit 0 |
| `cargo fmt --all -- --check` | Exit 0 |
| `just test-compile` | Exit 0; 59-file accepted program and 37-file store program clean |

The integration targets include the actual new regression file, existing
resolved-type tests, call-arity controls, generated WIT/current-evidence checks,
backend/Wasm encoding tests, privacy/boundary checks, corpus reachability and
robustness. No old raw evidence, golden output or compile-fail diagnostic was
rewritten to obtain these results.

Full-suite commands exceeded the review runner's execution window. All compiler
targets were then run to completion in separate batches of up to seven targets.
The incomplete attempts are not successful runs. On an unrestricted runner the
same set is reproduced with:

```sh
cargo test --locked -p pw-core --lib --tests
cargo clippy --locked -p pw-core --all-targets -- -D warnings
cargo fmt --all -- --check
just test-compile
```

The PR's own full CI and license/advisory results are separate evidence, not
inferred from these local compiler checks. No browser, network-lab, or performance
measurement was made in this repair.

## Intentional boundary

`DeclaredType` is written syntax, not the semantic authority. Its recursive tree
now reaches `ResolvedType` without reparsing or partially resolved arguments.
Legacy `Signature` consumers have not been merged with the unfinished dual-field
branch. Their complete atomic cutover remains next, followed by call unification
and declared-return compatibility.

The earlier nested-argument Blocked witness in `resolved_types.rs` was replaced
by a real unsupported higher-kinded parameter application. The distinction
between Blocked and Unresolved remains tested rather than deleted. These tests
do not certify every annotation use, nominal arity, callable generics, or E10-I.
