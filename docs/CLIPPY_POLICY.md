# Clippy policy

OpenFlight treats Clippy as a governed engineering surface. The root
`Cargo.toml` owns the active workspace lint baseline, `policy/clippy-lints.toml`
tracks the machine-readable policy and future Rust flips, and `cargo xtask
check-lint-policy` verifies that workspace crates inherit the shared policy.

## Baseline

The active baseline is intentionally workspace-wide:

- panic-free production and tests (`unwrap`, `expect`, `panic!`, `todo!`,
  `unimplemented!`, and `unreachable!` are denied);
- AST, UTF-8, string slicing, and indexing safety lints are denied;
- silent failure lints such as ignored futures, ignored `must_use` values, and
  discarded error mappings are denied;
- async/concurrency, unsafe/memory (with existing OpenFlight unsafe lanes staged
  as reviewed migration debt), file/path/process, API correctness, numeric, and
  reviewability lints are governed from the workspace root; and
- suppression governance lints reject silent `#[allow]`-style escapes.

Tests do not get carveouts. Test helpers should return `Result` or use explicit
assertion helpers instead of unchecked collapse or panic-driven setup.

## Suppressions

Prefer fixing the code. If a narrow exception is necessary, use `#[expect]` with
a reason that explains why the local shape is safe and when it should disappear:

```rust
#[expect(
    clippy::arithmetic_side_effects,
    reason = "Reviewed fixed-point lane; wrapping behavior is covered by property tests."
)]
fn mix_axis_sample(sample: i16, gain: i16) -> i16 {
    sample.wrapping_mul(gain)
}
```

Do not use broad `#[allow]` attributes or crate-level category suppressions.
Temporary repo debt belongs in `policy/clippy-debt.toml` with an owner, reason,
path, lint, and expiry.

## Upgrade ledger

OpenFlight currently ratchets to MSRV 1.93 for the strict lint policy. Planned
Rust 1.94 and 1.95 Clippy flips stay in `policy/clippy-lints.toml` until the
workspace MSRV reaches the activation version. The lint-policy gate fails if a
planned lint is activated too early or if the policy ledger drifts from the root
manifest.

## Repo-specific overlays

OpenFlight is a real-time, numeric, hardware, and simulator integration
workspace. Domain-specific additions may tighten the policy, especially around
real-time allocation, unsafe boundaries, numeric conversions, and process/file
footguns. Domain-specific weakenings must be represented as expiring debt, not
as `clippy.toml` test carveouts or broad workspace suppressions.
