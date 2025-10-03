# Flight Hub Governance

## Overview

This document establishes governance processes for Flight Hub development, ensuring architectural consistency, quality standards, and effective decision-making across the project.

## Architecture Decision Records (ADRs)

### Purpose

ADRs document significant architectural decisions and their rationale, providing:
- Historical context for design choices
- Guidance for future development
- Onboarding material for new contributors
- Basis for architectural reviews

### Process

1. **Proposal**: Create ADR draft with status "Proposed"
2. **Review**: Technical review by core team and stakeholders
3. **Decision**: Accept, reject, or request modifications
4. **Implementation**: Update status to "Accepted" and implement
5. **Evolution**: Mark as "Deprecated" or "Superseded" when replaced

### ADR Requirements

All ADRs must include:
- Clear problem statement and context
- Detailed decision with rationale
- Consequences (positive and negative)
- Alternatives considered
- Implementation notes where applicable

### Current ADRs

| ADR | Title | Status | Domain |
|-----|-------|--------|---------|
| [001](001-rt-spine-architecture.md) | Real-Time Spine Architecture | Accepted | Core |
| [002](002-writers-as-data.md) | Writers as Data Pattern | Accepted | Integration |
| [003](003-plugin-classes.md) | Plugin Classification System | Accepted | Extensibility |
| [004](004-zero-allocation-constraint.md) | Zero-Allocation Real-Time Constraint | Accepted | Performance |
| [005](005-pll-timing-discipline.md) | PLL-Based Timing Discipline | Accepted | Timing |
| [006](006-driver-light-approach.md) | Driver-Light Integration Approach | Accepted | Integration |
| [007](007-pipeline-ownership-model.md) | Pipeline Ownership Model | Accepted | Configuration |
| [008](008-ffb-mode-selection.md) | Force Feedback Mode Selection | Accepted | Safety |
| [009](009-safety-interlock-design.md) | Safety Interlock Design | Accepted | Safety |
| [010](010-schema-versioning-strategy.md) | Schema Versioning Strategy | Accepted | Compatibility |
| [011](011-observability-architecture.md) | Observability Architecture | Accepted | Operations |

## Architectural Principles

### Core Principles

1. **Safety First**: All decisions prioritize user safety, especially for force feedback
2. **Real-Time Guarantees**: Maintain deterministic timing under all conditions
3. **Boring Reliability**: Prefer proven, stable solutions over novel approaches
4. **Schema-First**: Define interfaces before implementation
5. **Zero-Allocation RT**: No memory allocation in real-time code paths

### Design Guidelines

1. **Isolation**: Clear boundaries between RT and non-RT code
2. **Composability**: Modular design with well-defined interfaces
3. **Testability**: All components must be unit and integration testable
4. **Observability**: Comprehensive logging and metrics without RT impact
5. **Graceful Degradation**: System continues operating under partial failures

## Quality Gates

### Mandatory Gates (Blocking)

All changes must pass these gates before merge:

#### Performance Gates
- **AX-Jitter**: 250Hz p99 ≤ 0.5ms on virtual + physical runner
- **HID-Latency**: HID write p99 ≤ 300μs on physical hardware
- **Zero-Allocation**: RT allocation counter must remain zero

#### Safety Gates
- **Soft-Stop**: USB disconnect → torque zero ≤ 50ms
- **Fault-Response**: All fault types trigger appropriate response ≤ 50ms
- **Interlock-Validation**: Physical interlock system prevents unauthorized high-torque

#### Compatibility Gates
- **Schema-Stability**: No breaking changes without version bump + migrator
- **Writers-Golden**: All golden file tests must pass
- **Blackbox-Integrity**: 10-minute capture with zero drops

### Advisory Gates (Warning)

These gates provide warnings but don't block:
- Code coverage below 80%
- Documentation coverage below 90%
- Performance regression >5%
- New unsafe code without justification

## Development Workflow

### Feature Development

1. **Requirements**: Define clear requirements with acceptance criteria
2. **Design**: Create or update relevant ADRs
3. **Implementation**: Follow ADR guidance and quality gates
4. **Testing**: Comprehensive unit, integration, and performance tests
5. **Documentation**: Update ADRs, READMEs, and API docs
6. **Review**: Technical review focusing on ADR compliance

### Breaking Changes

Breaking changes require:
1. **ADR Update**: Document the change and rationale
2. **Migration Path**: Provide clear upgrade instructions
3. **Deprecation Period**: Minimum 2 releases for major changes
4. **Compatibility Matrix**: Update supported version ranges

### Security Changes

Security-related changes require:
1. **Security Review**: Independent security assessment
2. **Threat Modeling**: Updated threat model if applicable
3. **Penetration Testing**: For significant security features
4. **Documentation**: Security implications clearly documented

## Code Organization

### Crate Structure

```
crates/
├── flight-core/          # Core types and utilities
├── flight-axis/          # Real-time axis processing (ADR-001, ADR-004)
├── flight-ffb/           # Force feedback engine (ADR-008, ADR-009)
├── flight-scheduler/     # RT scheduling (ADR-005)
├── flight-ipc/           # IPC layer (ADR-010)
├── flight-writers/       # Configuration management (ADR-002, ADR-006)
├── flight-*-adapter/     # Simulator adapters (ADR-006)
└── flight-service/       # Main service orchestration
```

### Ownership Model

Each crate has a designated owner responsible for:
- ADR compliance within their domain
- Code review and quality
- Performance characteristics
- API stability

#### Current Ownership

- **RT Spine** (axis, ffb, scheduler): Systems team
- **Adapters** (simconnect, xplane, dcs): Integration team  
- **UI/UX** (panels, streamdeck, ui): Application team
- **Infrastructure** (ipc, writers, service): Platform team

## Review Process

### Technical Reviews

All changes require review by:
1. **Crate Owner**: Domain expertise and ADR compliance
2. **Systems Reviewer**: For RT or safety-critical changes
3. **Security Reviewer**: For security-sensitive changes

### ADR Reviews

New or modified ADRs require:
1. **Technical Review**: By relevant domain experts
2. **Stakeholder Review**: By affected teams
3. **Architecture Review**: By architecture council
4. **Final Approval**: By project lead

## Metrics and Monitoring

### Key Metrics

- **Performance**: Jitter p99, latency p99, throughput
- **Quality**: Test coverage, bug density, security issues
- **Reliability**: MTBF, fault recovery time, availability
- **User Experience**: Setup time, error rates, support tickets

### Reporting

- **Weekly**: Performance dashboard with trend analysis
- **Monthly**: Quality metrics and technical debt assessment
- **Quarterly**: Architecture review and ADR assessment
- **Annually**: Full system architecture review

## Compliance and Auditing

### Regular Audits

- **Security Audit**: Annual third-party security assessment
- **Performance Audit**: Quarterly performance regression analysis
- **Code Quality Audit**: Monthly static analysis and review
- **ADR Compliance Audit**: Quarterly review of ADR adherence

### Documentation Requirements

- All public APIs must have rustdoc documentation
- All ADRs must be referenced in relevant crate READMEs
- All breaking changes must have migration guides
- All security features must have threat model documentation

## Conflict Resolution

### Technical Disagreements

1. **Discussion**: Open technical discussion with stakeholders
2. **Escalation**: Escalate to architecture council if unresolved
3. **Decision**: Architecture council makes binding decision
4. **Documentation**: Decision rationale documented in ADR

### Process Disputes

1. **Clarification**: Review governance documentation
2. **Discussion**: Open discussion with process stakeholders
3. **Amendment**: Propose governance changes if needed
4. **Approval**: Governance changes require consensus

## Evolution

This governance model evolves with the project:
- **Quarterly Reviews**: Assess governance effectiveness
- **Annual Updates**: Major governance model updates
- **Continuous Improvement**: Incorporate lessons learned
- **Community Feedback**: Regular stakeholder input

## References

- [ADR Template](README.md#adr-template)
- [Quality Gates Documentation](../quality-gates.md)
- [Security Guidelines](../security-guidelines.md)
- [Performance Standards](../performance-standards.md)