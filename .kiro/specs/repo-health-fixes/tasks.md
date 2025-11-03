# Implementation Plan

- [ ] 1. Fix aircraft auto-switch PhaseOfFlight classification
  - Reorder classification logic to prioritize high-energy phases (Cruise, Climb, Descent) before ground phases (Taxi, Park)
  - Ensure Taxi only matches when `on_ground` is true and `ground_speed < taxi_speed_max`
  - Ensure Cruise requires `alt_agl >= cruise_agl_min`, `vs.abs() <= cruise_vs_abs_max`, and `ias >= cruise_ias_min`
  - Place ground-only phases (Landing, Taxi, Park) at the end of the classification chain
  - _Requirements: 1.1, 1.5_

- [ ] 2. Add C172 test fixture for aircraft auto-switch tests
  - Add embedded C172 profile JSON under `#[cfg(test)]` in aircraft_switch.rs
  - Include pof_thresholds for cruise, climb, descent, approach, taxi, and takeoff
  - Modify `load_profile` function to check embedded test profiles when filesystem lookup fails
  - Use `id.eq_ignore_ascii_case("c172")` for case-insensitive matching
  - _Requirements: 1.2, 1.5_

- [ ] 3. Increment metrics counters on profile switch
  - Add `self.metrics.total_switches = self.metrics.total_switches.saturating_add(1)` in `commit_switch` method
  - Remove early return in `force_switch` that bypasses commit when target is same as current
  - Ensure `force_switch` always calls `commit_switch` to increment counter
  - _Requirements: 1.3, 1.4, 1.5_

- [ ] 4. Validate flight-core tests pass
  - Run `cargo test -p flight-core` and verify all tests pass
  - Specifically verify the 5 previously failing aircraft_switch tests now pass
  - Check that PhaseOfFlight classification tests produce expected phases
  - Check that metrics tests show non-zero switch counts
  - _Requirements: 1.5_

- [ ] 5. Fix flight-hid private_interfaces warning
  - Run `cargo public-api -p flight-hid` to check if `get_endpoint_state` is in public API
  - If method is not used externally: change visibility from `pub` to `pub(crate)`
  - If method is used externally: create `EndpointView<'a>` wrapper type and expose only needed methods
  - Run `cargo clippy -p flight-hid -- -Dwarnings` to verify warning is resolved
  - _Requirements: 2.1, 2.2, 2.3_

- [ ] 6. Investigate and fix flight-virtual abnormal exit
  - Run `cargo test -p flight-virtual -- --nocapture` with `RUST_BACKTRACE=1` and `RUST_LOG=debug`
  - Identify source of abnormal exit (spawned thread panic, channel error, timing issue, or Drop panic)
  - Fix identified issues:
    - Wrap spawned threads with `JoinHandle.join().expect("...")` in tests
    - Replace channel `unwrap()` with `expect("...")` or graceful error handling
    - Add bounded waits with timeouts for timing-dependent tests
  - Run tests again to verify clean completion
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [ ] 7. Create stable rustfmt.toml
  - Review current rustfmt.toml for nightly-only options (imports_granularity, group_imports, format_code_in_doc_comments, normalize_comments, wrap_comments)
  - Create new rustfmt.toml with only stable options: edition, max_width, use_small_heuristics, newline_style
  - Optionally create rustfmt.nightly.toml with nightly features for local development
  - Run `cargo fmt --all` to format all code including examples/
  - Run `cargo fmt --all -- --check` on stable Rust 1.89.0 to verify no warnings
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [ ] 8. Align workspace MSRV and edition
  - Update workspace Cargo.toml to specify `edition = "2024"` and `rust-version = "1.89.0"` in `[workspace.package]`
  - Verify individual crate Cargo.toml files inherit with `edition.workspace = true` and `rust-version.workspace = true`
  - Update README.md to accurately reflect edition 2024 and MSRV 1.89.0
  - Run `cargo check --all` to verify workspace configuration is valid
  - _Requirements: 5.1, 5.2, 5.3, 5.4_

- [ ] 9. Scope IPC bench lint allows
  - Find function-level `#[allow(unused_variables)]` in IPC bench code
  - Replace with parameter-level `#[cfg_attr(not(feature = "ipc-bench"), allow(unused_variables))]`
  - Find struct fields only used in benches/tests
  - Add field-level `#[cfg_attr(not(any(feature = "ipc-bench", test)), allow(dead_code))]`
  - Run `cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings` to verify
  - _Requirements: 6.1, 6.2, 6.3, 6.4_

- [ ] 10. Clean up meaningless test assertions
  - Search for `assert!(value >= 0)` where value is unsigned type (u32, u64, usize)
  - Either remove meaningless assertions or change to meaningful bounds (e.g., `assert!(value > 0)`)
  - Run `cargo test --all` to verify tests still pass
  - Run `cargo clippy --tests` to verify no unused_comparisons warnings
  - _Requirements: 8.1, 8.2, 8.3_

- [ ] 11. Harden CI workflows
  - Add concurrency control to workflow: `concurrency: { group: "ci-${{ github.workflow }}-${{ github.event.pull_request.number || github.sha }}", cancel-in-progress: true }`
  - Add timeout-minutes to jobs (30 for builds, 10 for tests)
  - Pin cargo-public-api version: `cargo install cargo-public-api --version 0.38.0`
  - Improve caching to include `~/.cargo/bin/cargo-public-api`
  - Verify required check names in repository settings match job names exactly
  - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5_

- [ ] 12. Create missing documentation files
  - Create stub ADR files: docs/adr/001-*.md through docs/adr/005-*.md with standard format (Status, Context, Decision, Consequences)
  - Create docs/regression-prevention.md with relevant testing and validation content
  - If using mdBook, update docs/SUMMARY.md to include all ADRs and documentation files
  - Verify all README links work by checking file existence
  - _Requirements: 9.1, 9.2, 9.3, 9.4_

- [ ] 13. Validate "properly working" definition
  - Run `cargo test -p flight-core` (must pass with 0 failures)
  - Run `cargo test -p flight-virtual` (must pass with no abnormal exit)
  - Run `cargo clippy -p flight-core -- -Dwarnings` (must pass)
  - Run `cargo clippy -p flight-hid -- -Dwarnings` (must pass)
  - Run `cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings` (must pass)
  - Run `cargo bench -p flight-ipc --features ipc-bench --no-run` (must compile)
  - Run `cargo bench -p flight-ipc --features "ipc-bench,ipc-bench-serde" --no-run` (must compile)
  - Run `cargo public-api -p flight-core --diff-git origin/main..HEAD` (should show only intended changes)
  - Run `cargo fmt --all -- --check` (must pass on stable)
  - Verify CI passes on both Windows and Linux
  - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 10.7, 10.8, 10.9, 10.10_

## Implementation Notes

### Commit Sequencing

For easier code review, consider organizing commits by category:

1. **Auto-switch fixes**: PhaseOfFlight reordering + C172 fixture + metrics increment
2. **HID interface**: private_interfaces resolution
3. **Virtual tests**: abnormal exit investigation and fixes
4. **Configuration**: rustfmt stable + MSRV/edition alignment
5. **Code quality**: IPC bench scoped allows + test assertion cleanup
6. **Infrastructure**: CI hardening
7. **Documentation**: ADRs and missing docs

This keeps related changes together and makes diffs easier to review.

### Testing Strategy

**Before starting**:
```bash
# Capture baseline
cargo test -p flight-core 2>&1 | tee test-baseline.log
cargo test -p flight-virtual 2>&1 | tee virtual-baseline.log
```

**After each fix**:
```bash
# Verify specific fix
cargo test -p <crate>
cargo clippy -p <crate> -- -Dwarnings
```

**Before marking complete**:
```bash
# Full validation
cargo test --all
cargo clippy --all -- -Dwarnings
cargo fmt --all -- --check
cargo public-api -p flight-core --diff-git origin/main..HEAD
```

### Debugging Tips

**For flight-virtual abnormal exit**:
- Use `--nocapture` to see stdout/stderr
- Set `RUST_BACKTRACE=1` for stack traces
- Set `RUST_LOG=debug` for detailed logging
- Run single test with `cargo test -p flight-virtual test_name -- --nocapture`
- Check for panics in Drop implementations
- Look for unwrap() on channels or thread joins

**For PhaseOfFlight classification**:
- Add debug prints to see which conditions match
- Create test cases for boundary conditions
- Verify threshold values make sense for aircraft type
- Check that ground detection (on_ground, ground_contact) is reliable

### Risk Mitigation

**Low Risk** (safe to implement immediately):
- Rustfmt configuration changes
- MSRV/edition alignment
- Documentation creation
- Test assertion cleanup

**Medium Risk** (test thoroughly):
- PhaseOfFlight reordering (changes classification logic)
- Metrics increment (changes observable behavior)
- IPC bench scoped allows (could hide real issues if done wrong)

**Higher Risk** (requires investigation first):
- flight-virtual abnormal exit (unknown root cause)
- flight-hid private_interfaces (depends on API usage)

### Success Criteria Checklist

Before marking the spec complete, verify:

- ✅ All flight-core tests pass (5 previously failing tests now pass)
- ✅ flight-virtual tests complete without abnormal exit
- ✅ flight-hid compiles without private_interfaces warnings
- ✅ rustfmt works on stable without warnings
- ✅ Workspace edition = "2024" and rust-version = "1.89.0"
- ✅ IPC bench lints use scoped allows
- ✅ CI has concurrency control and timeouts
- ✅ Test assertions are meaningful (no unused_comparisons)
- ✅ All documentation links work
- ✅ Repository meets "properly working" definition (all 10 criteria)
