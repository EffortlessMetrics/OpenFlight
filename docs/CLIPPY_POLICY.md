# Effortless Metrics Clippy Policy

OpenFlight treats Clippy as a governed engineering surface. The root
`Cargo.toml` owns the active workspace lint baseline; each workspace package
inherits that baseline with `[lints] workspace = true`; and `policy/` records
why the baseline exists, which future lints are planned, and where temporary
exceptions live.

## Goals

The policy exists to keep the real-time flight-control workspace safe and
reviewable without changing product behavior:

- panic-free production and test code;
- no swallowed futures, locks, results, parser errors, or file/process footguns;
- AST, UTF-8, string, slice, numeric, and unsafe-memory guardrails appropriate
  for simulator input, profile parsing, and 250 Hz real-time paths;
- explicit suppression receipts instead of broad local carveouts; and
- planned Rust 1.94 and 1.95 lint flips tracked before the MSRV bump.

## Active baseline

The active baseline is declared in `[workspace.lints.rust]` and
`[workspace.lints.clippy]` in the root manifest. It covers:

- panic-family lints such as `clippy::unwrap_used`, `clippy::expect_used`,
  `clippy::panic`, `clippy::todo`, `clippy::unimplemented`, and
  `clippy::unreachable`;
- silent-failure lints such as `clippy::let_underscore_future`,
  `clippy::unused_result_ok`, `clippy::map_err_ignore`, and
  `unused_must_use`;
- AST/string/slice safety lints such as `clippy::string_slice`,
  `clippy::indexing_slicing`, and `clippy::char_indices_as_byte_indices`;
- async/concurrency, unsafe/memory, numeric correctness, filesystem/process,
  API/trait correctness, and reviewability lints; and
- suppression governance lints such as `clippy::allow_attributes`,
  `clippy::allow_attributes_without_reason`, and
  `clippy::blanket_clippy_restriction_lints`.

`policy/clippy-lints.toml` is the machine-readable ledger for the same active
set. `cargo xtask check-lint-policy` verifies that active ledger entries match
the root manifest.

## No test carveouts

The workspace policy is panic-free, not merely production panic-free. Do not add
Clippy test carveouts to `clippy.toml`, including:

- `allow-unwrap-in-tests = true`
- `allow-expect-in-tests = true`
- `allow-panic-in-tests = true`
- `allow-indexing-slicing-in-tests = true`
- `allow-dbg-in-tests = true`

Prefer tests that return `Result` and use explicit error propagation from setup
and fixture parsing.

## Suppression style

Use narrow `#[expect(..., reason = "...")]` suppressions when a local exception
is genuinely required. Do not use broad `#[allow(...)]` suppressions. The
expectation should sit on the smallest item or expression that needs it, and
the reason must explain why the exception is safe and temporary or structurally
necessary.

Example:

```rust
#[expect(
    clippy::arithmetic_side_effects,
    reason = "RT tick counter intentionally wraps and is validated by scheduler tests."
)]
fn next_tick(counter: u64) -> u64 {
    counter.wrapping_add(1)
}
```

## Debt and allowlists

Temporary lint exceptions are tracked in `policy/clippy-debt.toml`. Every debt
entry must include a lint, path, owner, reason, and expiry date. Expired debt is
a policy failure.

Panic-family exceptions, when needed for migration, use
`policy/no-panic-allowlist.toml`. Identity is semantic:
`path + family + selector`; `last_seen` line and column values are advisory
locators only.

Non-Rust files that are part of the product, tooling, CI, or fixtures use
`policy/non-rust-allowlist.toml` entries with an owner, kind, reason, surface,
classification, and coverage command.

## Planned upgrade flips

The ledger tracks Rust 1.94 and 1.95 lints before the workspace activates them.
At MSRV 1.93, planned lints must remain in `policy/clippy-lints.toml` and must
not be active in `Cargo.toml`. When the MSRV moves to the activation version,
the planned entry should be promoted to the active manifest block and changed to
an active ledger entry in the same PR.

## Local verification

Run:

```bash
cargo xtask check-lint-policy
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The policy check verifies MSRV/ledger alignment, lint inheritance, root-manifest
lint coverage, absence of test carveouts, planned upgrade gates, suppression
style, and debt metadata.
