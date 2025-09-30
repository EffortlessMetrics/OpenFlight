# ADR-002: Writers as Data Pattern

## Status
Accepted

## Context

Flight simulators frequently update their configuration files, breaking integrations. Traditional approaches involve code changes for each sim update, leading to maintenance burden and delayed support for new versions. Users need reliable sim integration that survives updates.

## Decision

We implement "Writers as Data" - a table-driven approach where sim configurations are data, not code:

1. **Versioned JSON Diffs**: Each sim version has a JSON diff table describing required changes
2. **Golden File Testing**: CI validates expected behavior with fixture-based tests
3. **Verify/Repair Matrix**: Runtime validation detects drift and applies minimal corrections
4. **One-Click Rollback**: Users can instantly revert problematic changes

### Data Structure

```json
{
  "sim": "msfs",
  "version": "1.36.0",
  "diffs": [
    {
      "file": "MSFS/SimObjects/Airplanes/C172/panel.cfg",
      "section": "[ELECTRICAL]",
      "changes": {"light_nav": "1"}
    }
  ],
  "verify_tests": [
    {"action": "gear_toggle", "expect": "gear_light_change"}
  ]
}
```

## Consequences

### Positive
- Rapid response to sim updates (hours vs weeks)
- Automated validation prevents regressions
- Clear audit trail of changes
- User confidence with rollback capability

### Negative
- Initial complexity in test infrastructure
- Requires comprehensive fixture coverage
- More complex than direct code changes

## Alternatives Considered

1. **Direct Code Integration**: Rejected due to maintenance burden
2. **Plugin Architecture**: Rejected due to sim-specific constraints
3. **Configuration Templates**: Rejected due to lack of validation

## Implementation Details

- Golden tests run on every CI build
- Verify/Repair runs scripted sequences (gear/flap/AP)
- Coverage matrix tracks supported variables per sim
- Rollback preserves last 3 working configurations

## References

- Flight Hub Requirements: GI-01, GI-02, GI-05
- [Data-Driven Testing Patterns](https://example.com)