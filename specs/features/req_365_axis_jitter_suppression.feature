@REQ-365 @product
Feature: Axis Output Jitter Suppression  @AC-365.1
  Scenario: Changes smaller than jitter_threshold are ignored
    Given an axis with a jitter_threshold of 0.005
    When the axis input changes by less than 0.005 from the last output
    Then the output SHALL remain unchanged  @AC-365.2
  Scenario: Jitter threshold is configurable per axis with a default of 0.001
    Given an axis with no explicit jitter_threshold configured
    When the axis processes input
    Then it SHALL apply a default jitter threshold of 0.001  @AC-365.3
  Scenario: Suppression does not introduce non-monotonic behavior at threshold crossings
    Given an axis with jitter suppression enabled
    When inputs cross the threshold boundary in a monotonically increasing sequence
    Then the output SHALL not decrease at any point during the crossing  @AC-365.4
  Scenario: Property test verifies suppressed output never changes by less than threshold
    Given a property test with arbitrary axis inputs and a configured threshold
    When jitter suppression is applied to all inputs
    Then every output change SHALL be at least as large as the configured threshold  @AC-365.5
  Scenario: Jitter counter is incremented per suppressed sample
    Given an axis with jitter suppression enabled and a metrics counter attached
    When an input change below the threshold is suppressed
    Then the jitter suppression counter for that axis SHALL be incremented by one  @AC-365.6
  Scenario: Zero threshold disables suppression for backward compatibility
    Given an axis with jitter_threshold set to 0.0
    When any axis input change is processed
    Then all changes SHALL be passed through without suppression
