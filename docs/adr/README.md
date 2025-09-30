# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records for Flight Hub, documenting key architectural decisions and their rationale.

## ADR Index

| ADR | Title | Status |
|-----|-------|--------|
| [ADR-001](001-rt-spine-architecture.md) | Real-Time Spine Architecture | Accepted |
| [ADR-002](002-writers-as-data.md) | Writers as Data Pattern | Accepted |
| [ADR-003](003-plugin-classes.md) | Plugin Classification System | Accepted |
| [ADR-004](004-zero-allocation-constraint.md) | Zero-Allocation Real-Time Constraint | Accepted |
| [ADR-005](005-pll-timing-discipline.md) | PLL-Based Timing Discipline | Accepted |

## ADR Template

When creating new ADRs, use the following template:

```markdown
# ADR-XXX: [Title]

## Status
[Proposed | Accepted | Deprecated | Superseded by ADR-XXX]

## Context
[Describe the context and problem statement]

## Decision
[Describe the decision and rationale]

## Consequences
[Describe the positive and negative consequences]

## Alternatives Considered
[List alternatives that were considered]

## References
[Links to relevant documentation, discussions, etc.]
```

## Guidelines

- ADRs should be immutable once accepted
- Use clear, concise language
- Include rationale and trade-offs
- Reference ADRs in code and documentation
- Update status when decisions are superseded