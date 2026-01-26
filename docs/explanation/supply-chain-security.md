---
doc_id: DOC-SUPPLY-CHAIN-SECURITY
kind: explanation
area: infra
status: active
links:
  requirements: ["INF-REQ-9"]
  tasks: []
  adrs: []
---

# Flight Hub Supply Chain Security

This document describes the comprehensive supply chain security implementation for Flight Hub, addressing the requirements specified in task 37 and SEC-01.

## Overview

Flight Hub implements a multi-layered supply chain security approach that includes:

- **Dependency Auditing**: Continuous monitoring for security vulnerabilities
- **License Compliance**: Automated validation of third-party licenses
- **SPDX Integration**: Complete Software Package Data Exchange documentation
- **CI Security Gates**: Automated enforcement of security policies
- **Audit Trail**: Comprehensive documentation and tracking

## Security Tools and Configuration

### 1. Cargo Audit

**Purpose**: Detect known security vulnerabilities in dependencies

**Configuration**: Runs daily via CI and on every PR
- Database: RustSec Advisory Database
- Policy: Zero tolerance for security advisories
- Action: Build fails if vulnerabilities detected

**Usage**:
```bash
# Manual audit
cargo audit

# CI audit with strict policy
cargo audit --deny warnings
```

### 2. Cargo Deny

**Purpose**: Comprehensive dependency policy enforcement

**Configuration File**: `deny.toml`

**Policies Enforced**:
- **License Compliance**: Only approved licenses allowed
- **Banned Crates**: Known vulnerable or problematic crates blocked
- **Duplicate Dependencies**: Multiple versions of same crate prevented
- **Source Validation**: Only trusted registries and sources allowed

**Approved Licenses**:
- MIT
- Apache-2.0
- Apache-2.0 WITH LLVM-exception
- BSD-2-Clause
- BSD-3-Clause
- ISC
- Unicode-DFS-2016
- CC0-1.0

**Usage**:
```bash
# Run all checks
cargo deny check

# Check specific category
cargo deny check licenses
cargo deny check bans
cargo deny check advisories
cargo deny check sources
```

### 3. SPDX Implementation

**Purpose**: Complete software bill of materials and license tracking

**Components**:
- **Crate-level SPDX**: Each crate has SPDX identifiers in Cargo.toml
- **Source-level SPDX**: All source files have SPDX headers
- **SPDX Documents**: Machine-readable SPDX files for each crate

**SPDX Identifiers Used**:
```
SPDX-License-Identifier: MIT OR Apache-2.0
SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team
```

**Generated Documents**:
- `spdx/*.spdx`: Individual SPDX documents per crate
- `THIRD_PARTY_LICENSES.md`: Human-readable license summary
- `SUPPLY_CHAIN_AUDIT.md`: Comprehensive audit trail

## CI Security Gates

The CI pipeline implements multiple security gates that must pass before builds are approved:

### Gate 1: Security Advisory Check
- **Tool**: `cargo audit`
- **Policy**: Zero security advisories allowed
- **Action**: Build fails if vulnerabilities found

### Gate 2: Cargo Deny Compliance
- **Tool**: `cargo deny`
- **Policy**: All deny.toml policies must pass
- **Action**: Build fails on any policy violation

### Gate 3: License Compliance
- **Requirement**: ≥95% of dependencies must have approved licenses
- **Threshold**: ≤5 dependencies with unknown licenses
- **Action**: Build fails if thresholds exceeded

### Gate 4: SPDX Identifier Validation
- **Requirement**: All workspace crates must have SPDX identifiers
- **Check**: Validates Cargo.toml files for SPDX headers
- **Action**: Build fails if identifiers missing

### Gate 5: Dependency Count Validation
- **Threshold**: ≤150 direct dependencies
- **Rationale**: Maintain manageable dependency surface area
- **Action**: Build fails if threshold exceeded

### Gate 6: Audit Trail Validation
- **Requirement**: Audit documents must exist and be current
- **Files**: `SUPPLY_CHAIN_AUDIT.md`, `THIRD_PARTY_LICENSES.md`
- **Action**: Build fails if documents missing or outdated

### Gate 7: Third-Party License List
- **Requirement**: Complete license documentation
- **Validation**: Checks for required sections and formatting
- **Action**: Build fails if license list incomplete

## Automation Scripts

### Supply Chain Audit Script
**File**: `scripts/supply_chain_audit.rs`

**Functions**:
- Generates comprehensive dependency inventory
- Creates third-party license list
- Produces SPDX documents for all crates
- Creates audit trail documentation
- Validates license compliance

**Usage**:
```bash
cargo +nightly -Zscript scripts/supply_chain_audit.rs
```

### SPDX Identifier Script
**File**: `scripts/add_spdx_identifiers.rs`

**Functions**:
- Adds SPDX identifiers to all Cargo.toml files
- Adds SPDX headers to all source files
- Ensures consistent SPDX formatting

**Usage**:
```bash
cargo +nightly -Zscript scripts/add_spdx_identifiers.rs
```

### CI Security Gate Script
**File**: `scripts/ci_supply_chain_gate.rs`

**Functions**:
- Runs all security gates in CI environment
- Provides detailed pass/fail reporting
- Generates remediation guidance
- Enforces security policies

**Usage**:
```bash
cargo +nightly -Zscript scripts/ci_supply_chain_gate.rs
```

## Security Monitoring

### Daily Monitoring
- **Security Advisories**: Automated daily checks via CI
- **License Changes**: Monitored on every dependency update
- **Policy Violations**: Immediate CI failure and notification

### Quarterly Reviews
- **Dependency Audit**: Manual review of all dependencies
- **License Compliance**: Verification of license compatibility
- **Policy Updates**: Review and update security policies

### Incident Response
- **Vulnerability Detection**: Immediate notification and assessment
- **Remediation**: Coordinated response to security issues
- **Documentation**: Complete incident documentation

## Compliance and Reporting

### Audit Documentation
- **Supply Chain Audit**: `SUPPLY_CHAIN_AUDIT.md`
- **License List**: `THIRD_PARTY_LICENSES.md`
- **SPDX Documents**: `spdx/*.spdx`

### Compliance Metrics
- **License Compliance Rate**: Target ≥95%
- **Security Advisory Count**: Target = 0
- **Policy Violation Count**: Target = 0
- **Audit Trail Currency**: Target ≤7 days

### Reporting
- **CI Artifacts**: Audit documents uploaded with each build
- **Security Dashboard**: Metrics tracked in CI system
- **Compliance Reports**: Generated for security reviews

## Maintenance and Updates

### Tool Updates
- **Cargo Audit**: Updated automatically via CI
- **Cargo Deny**: Manual updates with policy review
- **Advisory Database**: Updated daily automatically

### Policy Updates
- **License Policy**: Reviewed quarterly
- **Banned Crates**: Updated as needed for security
- **Thresholds**: Adjusted based on project growth

### Documentation Updates
- **This Document**: Updated with policy changes
- **Audit Trail**: Generated automatically
- **License List**: Updated with dependency changes

## Integration with Development Workflow

### Pre-commit Hooks
- License validation on new dependencies
- SPDX identifier validation on new files

### Pull Request Checks
- Dependency review for license compliance
- Security advisory scanning
- Policy violation detection

### Release Process
- Complete supply chain audit
- License compliance verification
- Security clearance confirmation

## Troubleshooting

### Common Issues

**Security Advisory Failures**:
1. Run `cargo audit` to identify vulnerable crates
2. Update to patched versions if available
3. Replace with alternative crates if no patch
4. Document accepted risks in deny.toml with justification

**License Compliance Failures**:
1. Run `cargo deny check licenses` for details
2. Replace non-compliant dependencies
3. Update license policy if business justification exists
4. Document license exceptions with legal review

**SPDX Validation Failures**:
1. Run SPDX identifier script to add missing identifiers
2. Manually verify SPDX formatting
3. Ensure all new files have proper headers

**Dependency Count Violations**:
1. Review dependency tree with `cargo tree`
2. Remove unnecessary dependencies
3. Use feature flags to reduce optional dependencies
4. Consolidate similar functionality

## Security Contacts

For security-related questions or incidents:
- **Security Team**: security@flight-hub.dev
- **Supply Chain Issues**: supply-chain@flight-hub.dev
- **License Questions**: legal@flight-hub.dev

## References

- [SEC-01 Requirements](.kiro/specs/flight-hub/requirements.md)
- [Cargo Deny Documentation](https://embarkstudios.github.io/cargo-deny/)
- [RustSec Advisory Database](https://rustsec.org/)
- [SPDX Specification](https://spdx.github.io/spdx-spec/)
- [Supply Chain Security Best Practices](https://slsa.dev/)