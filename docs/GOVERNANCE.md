---
doc_id: DOC-META-GOVERNANCE
kind: reference
area: infra
status: active
links:
  requirements: []
  tasks: []
  adrs: []
---

# Project Governance

This document outlines how the OpenFlight project is governed, how decisions are made, and how contributors can participate.

## Core Values

1.  **Safety First**: FFB devices can cause physical injury. Safety features (interlocks, stops, ramps) are never compromised for convenience.
2.  **Real-Time Reliability**: "Boring-reliable" is the goal. We prioritize consistent timing and low jitter over new features.
3.  **Transparency**: Design decisions, roadmaps, and security audits are public.
4.  **Vendor Neutrality**: We support all hardware vendors equally via standard protocols (DirectInput/HID/USB).

## Decision Making

### Major Architectural Changes (RFCs)
Significant changes (e.g., changing the FFB engine topology, adding a new runtime dependency) require an **RFC (Request for Comments)** process:
1.  Open an issue or PR with the `RFC` tag.
2.  Draft a design document in `docs/design/` or `.kiro/specs/`.
3.  Solicit feedback from maintainers.
4.  Consensus must be reached before implementation begins.

### Day-to-Day Decisions
Routine decisions (refactoring, bug fixes, minor features) are handled via Code Review on Pull Requests.

## Roles

*   **Maintainers**: Have commit access and final say on architectural decisions. Responsible for release management and security.
*   **Contributors**: Submit PRs, report issues, and improve documentation.
*   **Users**: The pilots who use the software. Their safety and experience are paramount.

## Code of Conduct
We adhere to the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Be respectful, constructive, and kind.
