# Implementation Plan

- [x] 1. Fix aircraft auto-switch PhaseOfFlight classification





  - Reorder classification logic to prioritize high-energy phases (Cruise, Climb, Descent) before ground phases (Taxi, Park)
  - Ensure Taxi only matches when `on_ground` is true and `ground_speed < taxi_speed_max`
  - Ensure Cruise requires `alt_agl >= cruise_agl_min`, `vs.abs() <= cruise_vs_abs_max`, and `ias >= cruise_ias_min`
  - Place ground-only phases (Landing, Taxi, Park) at the end of the classification chain
  - _Requirements: 1.1, 1.5_

- [ ] 2. Add C172 test fixture for aircraft auto-switch tests
  - Create `tests/fixtures/profiles/` directory in flight-core
  - Create `tests/fixtures/profiles/C172.json` with pof_thresholds for cruise, climb, descent, approach, taxi, and takeoff
  - Add test helper function `test_profile_repo()` that returns ProfileRepo pointing to fixtures directory
  - Update tests to use `test_profile_repo()` instead of production profile paths
  - _Requirements: 1.4, 1.6_

- [ ] 3. Increment metrics counters on profile switch
  - Decide on semantics: count only on ID change (Option 1) or count on any force (Option 2)
  - Rename metric to `committed_switches` for clarity
  - In `commit_switch`, check if profile ID changed: `self.current_profile.as_ref().map(|p| &p.id) != Some(&new_profile.id)`
  - Use `checked_add(1).unwrap_or_else(|| { tracing::warn!("..."); 0 })` instead of `saturating_add` to detect overflow
  - Update tests to reflect chosen semantics
  - _Requirements: 1.5, 1.6_

- [ ] 4. Add hysteresis to PhaseOfFlight classification (optional but recommended)
  - Add `consecutive_frames` counter to classification state
  - Require N consecutive frames (e.g., 3-5 frames at 250Hz) meeting Cruise criteria before transitioning
  - Add unit tests for phase transition hysteresis (no flip-flop within M frames)
  - Add debug feature behind test cfg that logs which predicate matched for troubleshooting
  - _Requirements: 1.2_

- [ ] 5. Validate flight-core tests pass
  - Run `cargo test -p flight-core` and verify all tests pass
  - Specifically verify the 5 previously failing aircraft_switch tests now pass
  - Check that PhaseOfFlight classification tests produce expected phases
  - Check that metrics tests show correct switch counts based on chosen semantics
  - _Requirements: 1.6_

- [ ] 6. Fix flight-hid private_interfaces warning
  - Run `cargo public-api -p flight-hid diff origin/main..HEAD` to check current public API
  - If `get_endpoint_state` is not used externally: change visibility from `pub` to `pub(crate)`
  - If method is used externally: create `EndpointView<'a>` wrapper type and expose only needed methods (success_rate, avg_bytes_per_operation)
  - Optionally add deprecated shim if changing visibility: `#[deprecated] pub fn ... { self.internal_method() }`
  - Run `cargo clippy -p flight-hid -- -Dwarnings` to verify warning is resolved
  - _Requirements: 2.1, 2.2, 2.3_

- [ ] 7. Investigate and fix flight-virtual abnormal exit
  - Run `cargo test -p flight-virtual -- --nocapture` with `RUST_BACKTRACE=1` and `RUST_LOG=debug`
  - Identify source of abnormal exit (spawned thread panic, channel error, timing issue, or Drop panic)
  - Create reusable test helper: `wait_until(timeout: Duration, poll: impl Fn() -> bool)` for timing-dependent tests
  - Fix identified issues:
    - Wrap spawned threads with `JoinHandle.join().expect("Background thread panicked: ...")` in tests
    - Replace channel `unwrap()` with `expect("Receiver dropped unexpectedly")` or graceful error handling
    - Replace fixed `sleep()` calls with `wait_until` helper
    - Add final state assertion to verify no OS handle leaks
  - Run tests again to verify exit code 0 and no panics
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [ ] 8. Create stable rustfmt.toml
  - Review current rustfmt.toml for nightly-only options (imports_granularity, group_imports, format_code_in_doc_comments, normalize_comments, wrap_comments)
  - Create new rustfmt.toml with only stable options: edition, max_width, use_small_heuristics, newline_style
  - Optionally create rustfmt.nightly.toml with nightly features (gated behind opt-in CI job, non-required)
  - Run `cargo fmt --all` to format all code including examples/
  - Run `cargo fmt --all -- --check` on stable Rust 1.89.0 to verify no warnings
  - Ensure CI includes examples in formatting check
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

- [ ] 9. Align workspace MSRV and edition
  - Update workspace Cargo.toml to specify `edition = "2024"` and `rust-version = "1.89.0"` in `[workspace.package]`
  - Verify individual crate Cargo.toml files inherit with `edition.workspace = true` and `rust-version.workspace = true`
  - Update README.md to accurately reflect edition 2024 and MSRV 1.89.0 (note: let-chains do not require 2024 edition)
  - Run `cargo check --all` to verify workspace configuration is valid
  - Add dedicated MSRV CI job that builds and runs clippy on Rust 1.89.0 (Linux only)
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_

- [ ] 10. Scope IPC bench lint allows
  - Find function-level `#[allow(unused_variables)]` in IPC bench code
  - Replace with parameter-level `#[cfg_attr(not(feature = "ipc-bench"), allow(unused_variables))]`
  - Find struct fields only used in benches/tests
  - Add field-level `#[cfg_attr(not(any(feature = "ipc-bench", test)), allow(dead_code))]`
  - Run `cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings` to verify
  - _Requirements: 6.1, 6.2, 6.3, 6.4_

- [ ] 11. Clean up meaningless test assertions
  - Search for `assert!(value >= 0)` where value is unsigned type (u32, u64, usize)
  - Either remove meaningless assertions or change to meaningful bounds (e.g., `assert!(value > 0, "Duration should be non-zero")`)
  - For duration/timing assertions, consider reasonable range checks: `assert!(value > 0 && value < 10_000, "Duration {} ms outside expected range", value)`
  - Run `cargo test --all` to verify tests still pass
  - Run `cargo clippy --tests` to verify no unused_comparisons warnings
  - _Requirements: 8.1, 8.2, 8.3_

- [ ] 12. Harden CI workflows
  - Add concurrency control to workflow: `concurrency: { group: "ci-${{ github.workflow }}-${{ github.event.pull_request.number || github.sha }}", cancel-in-progress: true }`
  - Add platform-appropriate timeout-minutes: Windows tests 20min, Linux tests 10min, Windows builds 45min, Linux builds 30min
  - Set `fail-fast: false` in matrix jobs so Linux failures don't mask Windows issues
  - Pin cargo-public-api with --locked: `cargo install --locked cargo-public-api@=0.38.0`
  - Improve caching to include toolchain hash: `key: ${{ runner.os }}-${{ steps.rust-toolchain.outputs.cachekey }}-${{ hashFiles('**/Cargo.lock') }}`
  - Add step to get toolchain hash: `echo "cachekey=$(rustc -Vv | sha256sum | cut -d' ' -f1)" >> $GITHUB_OUTPUT`
  - Verify required check names in repository settings match job names exactly
  - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6_

- [ ] 13. Create missing documentation files
  - Create stub ADR files: docs/adr/001-*.md through docs/adr/005-*.md with standard format (Status, Context, Decision, Consequences)
  - Create docs/regression-prevention.md with relevant testing and validation content
  - If using mdBook, update docs/SUMMARY.md to include all ADRs and documentation files
  - Verify all README links work by checking file existence
  - _Requirements: 9.1, 9.2, 9.3, 9.4_

- [ ] 14. Validate "properly working" definition
  - Run `cargo test -p flight-core` (must pass with 0 failures)
  - Run `cargo test -p flight-virtual` (must pass with exit code 0, no panics in background threads)
  - Run `cargo clippy -p flight-core -- -Dwarnings` (must pass)
  - Run `cargo clippy -p flight-hid -- -Dwarnings` (must pass)
  - Run `cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings` (must pass)
  - Run `cargo bench -p flight-ipc --features ipc-bench --no-run` (must compile)
  - Run `cargo bench -p flight-ipc --features "ipc-bench,ipc-bench-serde" --no-run` (must compile)
  - Run `cargo public-api -p flight-core diff origin/main..HEAD` (use correct CLI: diff subcommand, not --diff flag)
  - Run `cargo public-api -p flight-hid diff origin/main..HEAD` (if API changes were made)
  - Run `cargo fmt --all -- --check` (must pass on stable)
  - Verify MSRV CI job passes (builds and runs clippy on Rust 1.89.0)
  - Verify CI passes on both Windows and Linux with appropriate timeouts
  - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 10.7, 10.8, 10.9, 10.10, 10.11_

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
