@REQ-423 @product
Feature: Input Dead-Band Around Center — Separate Center Deadzone from Edge Deadzone

  @AC-423.1
  Scenario: Center deadzone eliminates jitter around axis center
    Given an axis with a configured center deadzone of size cd
    When the raw input is within [-cd, cd]
    Then the output SHALL be exactly 0.0

  @AC-423.2
  Scenario: Edge deadzone eliminates false full-deflection signals at extremes
    Given an axis with a configured edge deadzone of size ed
    When the raw input is beyond [1.0 - ed, 1.0] or [-1.0, -1.0 + ed]
    Then the output SHALL be clamped to ±1.0

  @AC-423.3
  Scenario: Center and edge deadzones are independently configurable
    Given an axis profile with separate center_deadzone and edge_deadzone fields
    When the profile is loaded
    Then each deadzone SHALL be applied independently without coupling

  @AC-423.4
  Scenario: Property test — center deadzone output is 0 for inputs within [-cd, cd]
    Given any center_deadzone value cd in (0, 1) and any input in [-cd, cd]
    When the axis processes the input
    Then the output SHALL be exactly 0.0

  @AC-423.5
  Scenario: Property test — edge deadzone output is ±1 for inputs beyond the edge threshold
    Given any edge_deadzone value ed in (0, 0.5) and an input with |x| >= 1.0 - ed
    When the axis processes the input
    Then the output magnitude SHALL be exactly 1.0

  @AC-423.6
  Scenario: Zero deadzone values disable the respective deadzone for backward compatibility
    Given an axis with center_deadzone or edge_deadzone set to 0.0
    When the axis processes any input
    Then that deadzone SHALL have no effect (backward compatible behavior)
