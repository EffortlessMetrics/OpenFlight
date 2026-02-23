@REQ-34
Feature: Saitek panel type detection, LED management, and hardware verification

  @AC-34.1
  Scenario: Saitek panel type is detected correctly
    Given a Saitek panel device connected via USB
    When the panel type is detected from its product ID
    Then the correct panel variant SHALL be identified

  @AC-34.1
  Scenario: Saitek LED mappings are correct
    Given a Saitek panel variant
    When the LED mappings are queried
    Then each logical LED index SHALL map to the correct HID report byte and bit

  @AC-34.2
  Scenario: Saitek panel writer is created successfully
    Given a valid Saitek panel configuration
    When the panel writer is created
    Then the writer SHALL be ready to accept LED state updates

  @AC-34.2
  Scenario: Panel LED state management updates outputs
    Given a registered Saitek panel
    When LED states are set for individual LEDs
    Then the panel output SHALL reflect each LED state change

  @AC-34.2
  Scenario: Panel LED updates are rate limited
    Given a Saitek panel with rate limiting enabled
    When multiple LED updates arrive faster than the minimum interval
    Then only one update SHALL be forwarded per interval

  @AC-34.3
  Scenario: Verify matrix creation initializes correctly
    Given a Saitek panel verify matrix configuration
    When the matrix is created
    Then all verification slots SHALL be initialized to their default state

  @AC-34.3
  Scenario: Latency statistics are calculated correctly
    Given a series of latency samples recorded in the verify matrix
    When statistics are calculated
    Then mean and p99 latency SHALL be computed correctly

  @AC-34.3
  Scenario: Drift detection thresholds are applied
    Given a verify matrix with configured drift thresholds
    When panel drift is measured
    Then values within threshold SHALL pass and values outside SHALL trigger drift action
