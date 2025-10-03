# Implementation Plan

## Definitions

**Direct dependency (runtime):** A dependency in [dependencies] of any publishable workspace crate; excludes [dev-dependencies] and [build-dependencies]. Count via `cargo metadata` (not shell tools).

**HTTP unification scope:** The entire workspace graph must not contain hyper 0.14, native-tls, hyper-tls, or openssl—even transitively.

**Determinism:** Given the same Cargo.lock, THIRD_PARTY_LICENSES.md and SBOM outputs must be byte-for-byte identical; fail CI if Cargo.lock is dirty.

- [x] 1. Fix P0 license compliance issues





  - [x] 1.1 Update deny.toml with comprehensive license policy


    - Add Unicode-3.0, Unicode-DFS-2016, and MPL-2.0 to allowed licenses
    - Set exclude-dev-dependencies = true and include-build-dependencies = false
    - Add unicode-ident exception for compound license expressions
    - Configure unused-allowed-licenses = "warn"
    - _Requirements: SC-01.1, SC-01.2, SC-01.5_
  
  - [x] 1.2 Ensure examples crate compliance


    - Add license = "MIT OR Apache-2.0" to examples/Cargo.toml
    - Set publish = false and edition = "2024" in examples crate
    - _Requirements: SC-01.4_

- [x] 2. Implement HTTP stack unification





  - [x] 2.1 Add workspace patches and bans policy for HTTP unification


    - Add [patch.crates-io] section to root Cargo.toml with hyper 1.4, http 1.1, reqwest 0.12.8
    - Add deny.toml bans for hyper < 1.0.0, native-tls, hyper-tls, and openssl
    - Set multiple-versions = "deny" in deny.toml bans section
    - _Requirements: SC-02.1, SC-02.2, SC-02.5, SC-02.6_

  - [x] 2.2 Update workspace dependencies for HTTP unification


    - Update reqwest to 0.12.8 with default-features = false, features = ["rustls-tls", "http2", "json"]
    - Remove any native-tls or hyper-tls feature flags from existing dependencies
    - _Requirements: SC-02.2, SC-02.3_

  - [x] 2.3 Implement HTTP stack validation with negative proofs


    - Create CI validation commands: cargo tree -i native-tls | (! grep .)
    - Add validation for hyper-tls and openssl absence
    - Verify only hyper v1.x versions present with cargo tree -i hyper
    - _Requirements: SC-02.4, SC-02.5, SC-02.6_

- [x] 3. Update gRPC dependencies to unified versions





  - [x] 3.1 Update entire gRPC stack to 0.14.x with version alignment


    - Update workspace dependencies: tonic, prost, tonic-build, prost-build all to 0.14.x
    - Add prost-types if used, ensuring same minor version across all gRPC crates
    - Create gate check to verify all gRPC crates use identical 0.14.x versions
    - _Requirements: SC-03.1, SC-03.2, SC-03.5_

  - [x] 3.2 Configure tonic features based on transport usage


    - For built-in transport: tonic = { version = "0.14", features = ["transport", "codegen", "prost"] }
    - For custom transport: tonic = { version = "0.14", default-features = false, features = ["codegen", "prost"] }
    - Document transport configuration choice in workspace dependencies
    - _Requirements: SC-03.6_

- [x] 4. Update system-level dependencies





  - [x] 4.1 Update nix to 0.30 and migrate to typed file descriptors


    - Update nix version in workspace dependencies
    - Replace RawFd usage with OwnedFd/BorrowedFd in public APIs
    - _Requirements: SC-04.1, SC-04.5_

  - [x] 4.2 Update windows crate to 0.62


    - Update windows version and feature flags in workspace dependencies
    - Regenerate Windows bindings if using windows::build!
    - _Requirements: SC-04.2_

  - [x] 4.3 Add compiler warning enforcement for public API crates


    - Configure CI to use RUSTFLAGS="-Dwarnings" for public API crates only (flight-hid, flight-ipc, flight-service)
    - Add unit test or clippy lint to ensure public functions use OwnedFd/BorrowedFd/AsFd instead of RawFd
    - Keep dev ergonomics by not setting warnings globally
    - _Requirements: SC-04.4, SC-04.5_

- [x] 5. Implement enhanced CI gate controller





  - [x] 5.1 Update gate controller to use JSON parsing with tool pinning


    - Pin tool versions in CI: cargo-deny --locked --version 0.14.23, cargo-about --locked --version 0.6.4
    - Modify ci_supply_chain_gate.rs to use cargo deny --format json
    - Replace string parsing with structured JSON analysis using serde
    - Add lockfile guard: fail if Cargo.lock changed but not committed
    - _Requirements: SC-07.2, SC-07.4, NFR-A_

  - [x] 5.2 Fix exit code handling and warning classification


    - Implement proper exit code evaluation before parsing JSON output
    - Treat license-not-encountered as warnings, not failures in JSON diagnostics
    - Add retry logic only for transient execution failures, not policy failures
    - _Requirements: SC-07.1, SC-07.3_

  - [x] 5.3 Add dependency counting using cargo metadata


    - Implement runtime dependency counting via cargo metadata --format-version 1
    - Filter for kind == "normal" and target.is_none(), dedupe by package name
    - Exclude dev-dependencies and build-dependencies from count
    - _Requirements: SC-05.1, SC-05.3_

  - [x] 5.4 Implement duplicate major version detection


    - Add validation: cargo tree -d | rg -E "(axum|tower|hyper|thiserror|syn)" && exit 1 || true
    - Create automated check for single major versions across workspace
    - _Requirements: SC-05.2, SC-05.5_

  - [x] 5.5 Add comprehensive CI artifact retention


    - Store raw JSON outputs from cargo-deny, cargo-about, and cargo-audit
    - Include tool versions, execution environment (OS, rustc), and timestamps
    - Attach artifacts to every gate run for audit trail
    - _Requirements: SC-07.5_

- [x] 6. Implement cargo-about integration for license documentation












  - [x] 6.1 Create about.hjson configuration with license templates


    - Create about.hjson with accepted licenses including Unicode-3.0, Unicode-DFS-2016, MPL-2.0
    - Set ignore-dev-dependencies = true and ignore-build-dependencies = true
    - Commit full license texts for Unicode v3 and MPL-2.0 under licenses/ directory
    - _Requirements: SC-06.5_

  - [x] 6.2 Implement deterministic license document generation


    - Add lockfile validation: git diff --quiet Cargo.lock before generation
    - Pin cargo-about version --locked --version 0.6.4 for consistent output
    - Verify cargo about generate exits zero and git diff --exit-code is clean after generation
    - _Requirements: SC-06.6, NFR-A_

  - [x] 6.3 Generate comprehensive third-party license documentation


    - Include full Unicode and MPL-2.0 license texts in generated documentation
    - Add Unicode attribution note to THIRD_PARTY_LICENSES.md
    - Ensure all transitive dependency licenses are included
    - _Requirements: SC-06.1, SC-06.2, SC-06.3_

- [ ] 7. Add feature optimization for heavy dependencies
  - [ ] 7.1 Optimize tokio features in workspace dependencies
    - Replace tokio "full" features with minimal set: ["macros", "rt-multi-thread", "time", "fs", "signal", "sync"]
    - Add compile-fail test to prevent re-enabling "full" features at workspace root
    - Ensure criterion remains dev-only to keep dependency count stable
    - _Requirements: SC-05.4_

  - [ ] 7.2 Optimize tracing-subscriber and clap features
    - Set tracing-subscriber default-features = false with features = ["fmt", "env-filter"]
    - Set clap default-features = false with features = ["derive"]
    - _Requirements: SC-05.4_

- [ ] 8. Implement security policy enforcement
  - [ ] 8.1 Configure comprehensive registry and VCS source restrictions
    - Update deny.toml [sources] with unknown-registry = "deny" and unknown-git = "deny"
    - Set allow-registry = ["https://github.com/rust-lang/crates.io-index"]
    - Add empty allow-git template with per-crate exception structure
    - Create CI gate to fail on git dependencies or unknown registries
    - _Requirements: NFR-C_

  - [ ] 8.2 Add MSRV and edition enforcement across workspace
    - Create CI job to validate edition = "2024" in all workspace package Cargo.toml files
    - Verify rust-version = "1.89.0" consistency across all crates
    - Add automated check that fails CI if any crate deviates from workspace standards
    - _Requirements: NFR-B_

- [ ] 9. Add monitoring and metrics collection
  - [ ] 9.1 Implement gate execution time tracking with defined metrics
    - Add timing instrumentation to CI gate controller for scm.gate.duration_ms{gate="licenses"}
    - Track p95 execution time with target SLO of < 45s
    - Store last N runs to show trendlines for future dependency PRs
    - _Requirements: Monitoring metrics_

  - [ ] 9.2 Add dependency count and compliance monitoring
    - Implement scm.deps.direct_nondev_total gauge with threshold 150
    - Add scm.licenses.completeness gauge (1==complete)
    - Track scm.security.advisories_open_total gauge (0 target)
    - _Requirements: Monitoring metrics_

- [ ]* 9.3 Create supply chain health dashboard
  - Implement comprehensive metrics collection: scm.gate.status{gate=*}, scm.licenses.unified_http_ok
  - Add security advisory response time tracking and license compliance percentages
  - Create weekly and quarterly reporting automation with trend analysis
  - _Requirements: Monitoring and reporting_

## Task Completion Criteria (DoD)

**Task 1.1 DoD:** deny.toml allowlists Unicode/MPL, warns on unused allowances, runtime scope set; cargo deny check licenses passes.

**Task 2.1 DoD:** [patch.crates-io] present plus bans.deny for hyper<1, native-tls, hyper-tls, openssl; cargo deny check bans passes.

**Task 2.3 DoD:** native-tls|hyper-tls|openssl trees empty; hyper shows only v1; CI job http_unify green.

**Task 3.1 DoD:** All of prost, prost-build, tonic, tonic-build at 0.14.x in lockfile; compile + tests green.

**Task 5.1 DoD:** ci_supply_chain_gate reads JSON, fails on non-zero exit; attaches raw JSON as artifact.

**Task 6.2 DoD:** cargo-about generate produces identical output on re-run when Cargo.lock unchanged (git diff --exit-code clean).

**Task 8.1 DoD:** cargo deny check sources fails on git/unknown registries; CI green with crates.io only.