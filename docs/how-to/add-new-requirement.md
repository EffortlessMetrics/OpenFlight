---
doc_id: DOC-HOWTO-ADD-REQUIREMENT
kind: how-to
area: infra
status: active
links:
  requirements: [INF-REQ-2]
  tasks: []
  adrs: []
---

# How to Add a New Requirement

This guide explains how to add a new requirement to the Flight Hub project using the spec ledger system.

## Overview

Requirements in Flight Hub are tracked in the spec ledger (`specs/spec_ledger.yaml`), which provides machine-readable traceability from requirements to acceptance criteria to tests.

## Step-by-Step Process

### 1. Determine Requirement Type

Choose the appropriate requirement ID prefix:

- **Product Requirements**: Use `REQ-N` for features and functionality
- **Infrastructure Requirements**: Use `INF-REQ-N` for tooling, documentation, and process

### 2. Add to Spec Ledger

Edit `specs/spec_ledger.yaml` and add your requirement:

```yaml
requirements:
  - id: REQ-15  # or INF-REQ-13 for infrastructure
    name: Short descriptive name
    status: draft
    ac:
      - id: AC-15.1
        description: "WHEN condition THEN system SHALL behavior"
        tests: []
      - id: AC-15.2
        description: "WHEN condition THEN system SHALL behavior"
        tests: []
```

### 3. Write Acceptance Criteria

Each requirement should have 2-5 acceptance criteria. Follow the EARS pattern:

**EARS Patterns:**
- **Ubiquitous**: THE system SHALL response
- **Event-driven**: WHEN trigger, THE system SHALL response
- **State-driven**: WHILE condition, THE system SHALL response
- **Unwanted event**: IF condition, THEN THE system SHALL response
- **Optional feature**: WHERE option, THE system SHALL response

**Example:**
```yaml
- id: AC-15.1
  description: "WHEN a user connects a new device THEN the system SHALL detect it within 100ms"
  tests: []
```

### 4. Set Initial Status

Use the appropriate status:

- `draft`: Requirement defined but not implemented
- `implemented`: Code written but tests incomplete
- `tested`: All acceptance criteria have passing tests
- `deprecated`: No longer applicable

Start with `draft` for new requirements.

### 5. Validate the Schema

Ensure your changes conform to the schema:

```bash
cargo xtask validate
```

This will check:
- YAML syntax
- Required fields present
- ID format correctness
- Status values valid

### 6. Create Documentation

Create or update documentation that references the requirement:

```bash
# Create a concept doc
touch docs/concepts/my-feature.md
```

Add front matter with the requirement link:

```markdown
---
doc_id: DOC-MY-FEATURE
kind: concept
area: flight-core
status: draft
links:
  requirements: [REQ-15]
  tasks: []
  adrs: []
---

# My Feature Concepts

[Content describing the feature...]
```

### 7. Create Gherkin Scenarios (Optional but Recommended)

Create a feature file for executable specifications:

```bash
touch specs/features/req_15_my_feature.feature
```

Write scenarios with tags:

```gherkin
@REQ-15
Feature: My Feature

  @AC-15.1
  Scenario: Device detection
    Given a system with no devices connected
    When a new device is connected
    Then the system SHALL detect it within 100ms

  @AC-15.2
  Scenario: Device configuration
    Given a detected device
    When configuration is applied
    Then the device SHALL respond with acknowledgment
```

### 8. Link Tests as They're Written

As you implement and test the requirement, add test references:

```yaml
- id: AC-15.1
  description: "WHEN a user connects a new device THEN the system SHALL detect it within 100ms"
  tests:
    - flight-hid::device::tests::test_device_detection_latency
    - feature: specs/features/req_15_my_feature.feature:Scenario: Device detection
```

### 9. Update Status

As implementation progresses, update the status:

```yaml
- id: REQ-15
  name: Device Hotplug Detection
  status: implemented  # Changed from draft
  ac:
    # ...
```

When all acceptance criteria have tests:

```yaml
status: tested
```

### 10. Generate Reports

Check the requirement status:

```bash
# Generate feature status report
cargo xtask ac-status

# View the report
cat docs/feature_status.md
```

## Test Reference Formats

### Simple Format

For unit tests:

```yaml
tests:
  - flight-core::module::tests::test_name
```

### Feature Reference

For Gherkin scenarios:

```yaml
tests:
  - feature: specs/features/req_15_my_feature.feature:Scenario: Device detection
```

### Command Reference

For benchmark or integration tests:

```yaml
tests:
  - command: cargo bench --bench device_latency
```

### Object Format

For complex references:

```yaml
tests:
  - test: flight-core::device::tests::test_detection
  - feature: specs/features/req_15_my_feature.feature:Scenario: Device detection
  - command: cargo test --test device_integration
```

## Validation Checklist

Before committing, verify:

- [ ] Requirement ID follows pattern (REQ-N or INF-REQ-N)
- [ ] Acceptance criteria IDs follow pattern (AC-N.M)
- [ ] All acceptance criteria use EARS patterns
- [ ] Status is one of: draft, implemented, tested, deprecated
- [ ] `cargo xtask validate` passes
- [ ] Documentation links to the requirement
- [ ] Gherkin scenarios created (if applicable)

## Common Mistakes

### ❌ Vague Acceptance Criteria

```yaml
description: "The system should work well"
```

### ✅ Specific and Testable

```yaml
description: "WHEN processing 1000 events THEN the system SHALL complete within 100ms p99"
```

### ❌ Missing EARS Pattern

```yaml
description: "Fast device detection"
```

### ✅ Proper EARS Format

```yaml
description: "WHEN a device is connected THEN the system SHALL detect it within 100ms"
```

### ❌ Wrong Status for Tested Requirement

```yaml
status: tested
ac:
  - id: AC-15.1
    description: "..."
    tests: []  # No tests!
```

### ✅ Correct Status

```yaml
status: draft  # Or implemented, but not tested without tests
```

## Related Requirements

This guide implements **INF-REQ-2: Spec Ledger and Traceability**, which specifies the requirements for machine-readable requirement tracking.

## Related Documentation

- [Spec Ledger Schema](../../schemas/spec_ledger.schema.json)
- [Requirements Overview](../requirements/overview.md)
- [How to Run Tests](./run-tests.md)

