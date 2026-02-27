@REQ-327 @product
Feature: Conditional Axis Mapping  @AC-327.1
  Scenario: Axis mapping can be conditional on flight phase or gear state
    Given a profile with a conditional axis mapping tied to gear-up state
    When the gear is retracted in the simulator
    Then the service SHALL switch to the gear-up axis mapping for that axis  @AC-327.2
  Scenario: Profile rules can switch axis assignments based on sim variable
    Given a profile rule that maps a spoiler axis conditionally on a sim variable
    When the sim variable crosses the configured threshold
    Then the service SHALL apply the alternative axis assignment defined in the rule  @AC-327.3
  Scenario: Conditional mappings have priority order (first-match wins)
    Given a profile with multiple overlapping conditional mappings for the same axis
    When several conditions are simultaneously true
    Then the service SHALL apply only the highest-priority (first-listed) matching mapping  @AC-327.4
  Scenario: Conditions are evaluated on each tick without allocation
    Given the RT spine is processing at 250Hz with conditional axis mappings active
    When condition evaluation runs on each tick
    Then no heap allocations SHALL occur during condition evaluation  @AC-327.5
  Scenario: Default unconditional mapping is always present
    Given a profile with conditional mappings but no matching condition is active
    When the axis is processed
    Then the service SHALL apply the default unconditional mapping  @AC-327.6
  Scenario: Condition evaluation errors fall back to default mapping
    Given a conditional mapping whose condition references an unavailable sim variable
    When the condition is evaluated
    Then the service SHALL fall back to the default mapping and log a warning
