# Requirements Document

## Introduction

This feature addresses critical gaps preventing the repository from reaching a "properly working" state. The current state includes 5 failing tests in flight-core's aircraft_switch module, API hygiene issues in flight-hid, potential abnormal exits in flight-virtual, and various configuration/documentation inconsistencies. The goal is to systematically fix these issues to achieve green CI, usable binaries, and sane defaults across all platforms.

## Glossary

- **PhaseOfFlight (PoF)**: Enum representing aircraft flight phases (Taxi, Takeoff, Climb, Cruise, Descent, Approach, Landing, Park, Emergency)
- **flight-core**: Core crate containing shared functionality including aircraft auto-switching logic
- **flight-virtual**: Virtual device simulation crate for testing
- **flight-hid**: Hardware interface device crate with platform-specific implementations
- **MSRV**: Minimum Supported Rust Version (currently 1.89.0)
- **Edition**: Rust language edition (2021 or 2024)
- **private_interfaces**: Rust compiler warning when public API exposes private types
- **rustfmt**: Rust code formatter with stable and nightly feature sets
- **ADR**: Architecture Decision Record - documentation of key technical decisions

## Requirements

### Requirement 1

**User Story:** As a developer, I want all flight-core tests to pass, so that I can trust the aircraft auto-switching logic works correctly

#### Acceptance Criteria

1. WHEN the PhaseOfFlight classification logic evaluates flight conditions, THEN the System SHALL prioritize high-energy phases (Cruise, Climb, Descent) over ground phases (Taxi, Park)
2. WHEN Cruise classification is evaluated, THEN the System SHALL require N consecutive frames meeting (agl >= min AND |vs| <= max AND ias >= min) to prevent phase flapping
3. WHEN Taxi classification is evaluated, THEN the System SHALL require on_ground == true AND ground_contact_duration >= 150ms AND ground_speed < taxi_speed_max
4. WHEN test fixtures require aircraft profiles, THEN the Test System SHALL provide fixture-based profiles via test repo or in-memory provider (no embedded JSON in library crate)
5. WHEN the auto-switch system commits a profile change, THEN the Metrics System SHALL increment the committed_switches counter only when the active profile pointer changes OR a force operation explicitly records a committed switch
6. WHEN the developer runs `cargo test -p flight-core`, THEN the Test System SHALL pass all tests with zero failures

### Requirement 2

**User Story:** As a developer, I want flight-hid's public API to be clean, so that the crate compiles without private_interfaces warnings

#### Acceptance Criteria

1. WHERE flight-hid exposes public methods returning private types, THE flight-hid Crate SHALL either lower method visibility to `pub(crate)` or wrap private types in opaque public views
2. WHEN the developer runs `cargo clippy -p flight-hid -- -Dwarnings`, THEN the Clippy System SHALL complete without private_interfaces warnings
3. THE flight-hid Crate SHALL NOT expose internal platform-specific types (e.g., Windows HANDLE) through public API

### Requirement 3

**User Story:** As a developer, I want flight-virtual tests to complete reliably, so that I can validate virtual device behavior without mysterious failures

#### Acceptance Criteria

1. WHEN the developer runs `cargo test -p flight-virtual -- --nocapture` with RUST_BACKTRACE=1, THEN the Test System SHALL complete with exit code 0
2. WHERE background tasks spawn threads, THE Test System SHALL assert all JoinHandle::join() results are Ok (no panics in background threads)
3. WHERE background tasks use channels, THE Test System SHALL use expect("...") with context instead of bare unwrap() on send/recv operations
4. WHERE tests depend on timing, THE Test System SHALL use a reusable wait_until(timeout, poll) helper instead of fixed sleep() calls
5. WHEN all flight-virtual tests complete, THEN the Test System SHALL have no OS handle leaks (verified by final state assertion)

### Requirement 4

**User Story:** As a developer, I want rustfmt to work cleanly on stable Rust, so that I can format code without nightly-only warnings

#### Acceptance Criteria

1. WHERE rustfmt.toml contains nightly-only options, THE Configuration System SHALL remove or comment out unstable options for stable builds
2. WHEN the developer runs `cargo fmt --all -- --check` on stable Rust 1.89.0, THEN the Formatter SHALL complete without warnings about unknown configuration options
3. THE Repository SHALL optionally provide rustfmt.nightly.toml for developers who want nightly formatting features (gated behind opt-in CI job)
4. WHERE example code exists in examples/ directory, THE Formatter SHALL format those files consistently with the main codebase
5. THE CI System SHALL include examples in `cargo fmt --all -- --check` gate to prevent formatting drift

### Requirement 5

**User Story:** As a developer, I want workspace configuration to be consistent, so that MSRV and edition settings align across all documentation and configuration files

#### Acceptance Criteria

1. WHERE the README states "Rust 1.89.0 MSRV", THE Workspace Cargo.toml SHALL specify `rust-version = "1.89.0"`
2. THE Workspace Cargo.toml SHALL specify `edition = "2024"` for consistency (note: let-chains do not require 2024 edition)
3. WHERE individual crate Cargo.toml files exist, THEY SHALL inherit workspace edition and rust-version settings
4. THE README SHALL accurately reflect the edition and MSRV specified in Cargo.toml
5. THE CI System SHALL include a dedicated MSRV job that builds and runs clippy on Rust 1.89.0 to prevent accidental MSRV bumps

### Requirement 6

**User Story:** As a developer, I want IPC bench lints to remain strict, so that code quality doesn't degrade over time

#### Acceptance Criteria

1. WHERE unused variables exist only for specific feature configurations, THE Code SHALL use parameter-level `#[cfg_attr(..., allow(unused_variables))]` instead of function-level allows
2. WHERE struct fields are only used in benches or tests, THE Code SHALL use `#[cfg_attr(not(any(feature = "ipc-bench", test)), allow(dead_code))]` on those fields
3. WHEN the developer runs `cargo clippy -p flight-ipc --benches --features ipc-bench -- -Dwarnings`, THEN the Clippy System SHALL pass without warnings
4. THE IPC Crate SHALL NOT accumulate broad allow() attributes that hide genuine issues

### Requirement 7

**User Story:** As a developer, I want CI workflows to be robust and efficient, so that builds don't hang or waste resources

#### Acceptance Criteria

1. WHERE CI workflows can have concurrent runs, THE Workflow Configuration SHALL include concurrency groups with cancel-in-progress for PR builds
2. WHERE CI jobs can timeout, THE Workflow Configuration SHALL specify platform-appropriate timeout values (Windows tests: 20min, Linux tests: 10min, Windows builds: 45min, Linux builds: 30min)
3. WHERE CI uses external tools like cargo-public-api, THE Workflow Configuration SHALL pin tool versions with --locked flag (e.g., cargo install --locked cargo-public-api@=0.38.0)
4. WHERE CI has required checks, THE Repository Settings SHALL match job names exactly to prevent merge gate bypasses
5. THE CI System SHALL cache cargo registry, target directory, and installed tools with cache keys including toolchain hash
6. THE CI System SHALL set fail-fast: false in matrix jobs so Linux failures don't mask Windows issues

### Requirement 8

**User Story:** As a developer, I want test assertions to be meaningful, so that I can understand what's being validated

#### Acceptance Criteria

1. WHERE tests assert on unsigned values with `>= 0`, THE Test Code SHALL either remove meaningless assertions or change to meaningful bounds (e.g., `> 0`)
2. WHERE tests have unused_comparisons warnings, THE Test Code SHALL fix or remove those comparisons
3. WHEN the developer runs `cargo test --all`, THEN the Test System SHALL complete without compiler warnings about test code

### Requirement 9

**User Story:** As a developer, I want documentation links to work, so that I can navigate to referenced materials

#### Acceptance Criteria

1. WHERE the README references ADR documents (docs/adr/001-... through 005-...), THOSE Files SHALL exist or be stubbed with one-page summaries
2. WHERE the README references docs/regression-prevention.md, THAT File SHALL exist with relevant content
3. WHERE documentation uses mdBook or similar, THE Build System SHALL include all referenced documents in SUMMARY.md
4. THE Documentation System SHALL NOT have broken internal links

### Requirement 10

**User Story:** As a developer, I want a clear definition of "properly working", so that I know when the repository is ready for production use

#### Acceptance Criteria

1. WHEN all core tests run, THEN `cargo test -p flight-core` SHALL pass with zero failures
2. WHEN all virtual tests run, THEN `cargo test -p flight-virtual` SHALL pass with exit code 0 and no panics in background threads
3. WHEN linting runs on changed crates (path-gated), THEN `cargo clippy -- -Dwarnings` SHALL pass for affected crates
4. WHEN IPC benches compile, THEN both feature modes SHALL compile successfully with `--no-run`
5. WHEN public API is checked, THEN `cargo public-api -p <crate> diff origin/main..HEAD` SHALL show only intended changes (use correct CLI: diff subcommand, not --diff flag)
6. WHEN formatting is checked, THEN `cargo fmt --all -- --check` SHALL pass on stable Rust without warnings
7. WHEN CI runs on Windows and Linux, THEN all required jobs SHALL pass on both platforms
8. WHEN CI jobs are path-gated, THEN only relevant jobs SHALL run for specific file changes
9. WHEN CI jobs have timeouts, THEN no job SHALL hang indefinitely (platform-appropriate timeouts set)
10. WHEN MSRV is validated, THEN a dedicated CI job SHALL build and run clippy on Rust 1.89.0 (Linux only) to catch accidental bumps
11. THE Repository SHALL meet all above criteria before being considered "properly working"

## Success Criteria

- ✅ All 5 failing aircraft_switch tests pass
- ✅ flight-hid compiles without private_interfaces warnings
- ✅ flight-virtual tests complete without abnormal exits
- ✅ rustfmt works cleanly on stable Rust 1.89.0
- ✅ Workspace edition and MSRV are consistent across all files
- ✅ IPC bench lints remain strict with scoped allows
- ✅ CI workflows have concurrency control and timeouts
- ✅ Test assertions are meaningful (no unused_comparisons)
- ✅ All documentation links work (ADRs, regression-prevention.md)
- ✅ Repository meets the "properly working" definition of done

## Out of Scope

- Performance optimization beyond compilation fixes
- New feature development
- Refactoring that changes behavior
- Breaking API changes
- Migration to different testing frameworks
- Comprehensive integration test suites (focus on unit tests)
