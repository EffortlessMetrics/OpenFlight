# Implementation Plan

- [ ] 1. Fix P0 license compliance issues
  - Update deny.toml to allow Unicode and MPL licenses with proper configuration
  - Add unicode-ident exception for compound license expressions
  - Ensure examples crate has proper license field and publish = false
  - _Requirements: SC-01.1, SC-01.2, SC-01.4_

- [ ] 2. Implement HTTP stack unification
  - [ ] 2.1 Add workspace patches to enforce single HTTP stack versions
    - Add [patch.crates-io] section to root Cargo.toml
    - Pin hyper to 1.4, http to 1.1, and reqwest to 0.12.8 with rustls
    - _Requirements: SC-02.1, SC-02.2, SC-02.5, SC-02.6_

  - [ ] 2.2 Update workspace dependencies for HTTP unification
    - Update reqwest to 0.12 with rustls-tls features in workspace dependencies
    - Remove any native-tls or hyper-tls feature flags from existing dependencies
    - _Requirements: SC-02.2, SC-02.3_

  - [ ] 2.3 Implement HTTP stack validation commands
    - Create validation script to check for banned HTTP dependencies
    - Add negative proof commands for native-tls, hyper-tls, and openssl
    - _Requirements: SC-02.5, SC-02.6_

- [ ] 3. Update gRPC dependencies to unified versions
  - [ ] 3.1 Update tonic and prost to 0.14.x across workspace
    - Update workspace dependencies for tonic, prost, tonic-build, prost-build
    - Ensure all gRPC-related crates use the same minor version
    - _Requirements: SC-03.1, SC-03.2, SC-03.5_

  - [ ] 3.2 Configure tonic features for custom transport compatibility
    - Set tonic default-features = false with specific features array
    - Add codegen and prost features while maintaining transport compatibility
    - _Requirements: SC-03.6_

- [ ] 4. Update system-level dependencies
  - [ ] 4.1 Update nix to 0.30 and migrate to typed file descriptors
    - Update nix version in workspace dependencies
    - Replace RawFd usage with OwnedFd/BorrowedFd in public APIs
    - _Requirements: SC-04.1, SC-04.5_

  - [ ] 4.2 Update windows crate to 0.62
    - Update windows version and feature flags in workspace dependencies
    - Regenerate Windows bindings if using windows::build!
    - _Requirements: SC-04.2_

  - [ ] 4.3 Add compiler warning enforcement for public API crates
    - Configure CI to use RUSTFLAGS="-Dwarnings" for public API crates
    - Ensure builds pass without deprecated warnings
    - _Requirements: SC-04.4_

- [ ] 5. Implement enhanced CI gate controller
  - [ ] 5.1 Update gate controller to use JSON parsing
    - Modify ci_supply_chain_gate.rs to use cargo deny --format json
    - Replace string parsing with structured JSON analysis
    - _Requirements: SC-07.2, SC-07.4_

  - [ ] 5.2 Fix exit code handling and warning classification
    - Implement proper exit code evaluation before parsing output
    - Treat license-not-encountered as warnings, not failures
    - _Requirements: SC-07.1, SC-07.3_

  - [ ] 5.3 Add dependency counting with runtime-only scope
    - Implement dependency counting using cargo metadata with normal deps only
    - Exclude dev-dependencies and build-dependencies from count
    - _Requirements: SC-05.1, SC-05.3_

  - [ ] 5.4 Implement duplicate major version detection
    - Add validation for single major versions of axum, tower, hyper, thiserror, syn
    - Create automated check using cargo tree -d with regex filtering
    - _Requirements: SC-05.2, SC-05.5_

  - [ ] 5.5 Add CI artifact retention for audit trail
    - Store raw JSON outputs from cargo-deny and cargo-about as CI artifacts
    - Include tool versions and execution environment in artifacts
    - _Requirements: SC-07.5_

- [ ] 6. Implement cargo-about integration for license documentation
  - [ ] 6.1 Create about.hjson configuration file
    - Configure cargo-about with proper license acceptance list
    - Set ignore-dev-dependencies and ignore-build-dependencies to true
    - _Requirements: SC-06.5_

  - [ ] 6.2 Implement deterministic license document generation
    - Add lockfile validation before generation
    - Pin cargo-about version for consistent output
    - _Requirements: SC-06.6_

  - [ ] 6.3 Generate comprehensive third-party license documentation
    - Include full Unicode and MPL-2.0 license texts
    - Add Unicode attribution note to documentation
    - _Requirements: SC-06.1, SC-06.2, SC-06.3_

- [ ] 7. Add feature optimization for heavy dependencies
  - [ ] 7.1 Optimize tokio features in workspace dependencies
    - Replace "full" features with minimal required feature set
    - Use specific features like "macros", "rt-multi-thread", "time", "fs", "signal", "sync"
    - _Requirements: SC-05.4_

  - [ ] 7.2 Optimize tracing-subscriber and clap features
    - Set default-features = false for tracing-subscriber with specific features
    - Minimize clap features to "derive" only
    - _Requirements: SC-05.4_

- [ ] 8. Implement security policy enforcement
  - [ ] 8.1 Configure registry and VCS source restrictions
    - Update deny.toml [sources] section to deny unknown registries and git sources
    - Allow only crates.io registry in allow-registry list
    - _Requirements: NFR-C_

  - [ ] 8.2 Add MSRV and edition enforcement
    - Create CI check to validate edition = "2024" across workspace packages
    - Verify rust-version = "1.89.0" consistency in all crates
    - _Requirements: NFR-B_

- [ ] 9. Add monitoring and metrics collection
  - [ ] 9.1 Implement gate execution time tracking
    - Add timing instrumentation to CI gate controller
    - Track p95 execution time with target SLO of < 45s
    - _Requirements: Monitoring metrics_

  - [ ] 9.2 Add dependency count trend monitoring
    - Implement tracking for direct non-dev dependency count over time
    - Set alert threshold at 150 dependencies
    - _Requirements: Monitoring metrics_

- [ ]* 9.3 Create supply chain health dashboard
  - Implement metrics collection for license compliance percentages
  - Add security advisory response time tracking
  - Create weekly and quarterly reporting automation
  - _Requirements: Monitoring and reporting_