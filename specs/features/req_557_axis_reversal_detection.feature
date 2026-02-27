@REQ-557 @product
Feature: Axis Reversal Detection — Service should detect when an axis is accidentally reversed

  @AC-557.1
  Scenario: Reversal detection checks expected control response direction
    Given an axis with an expected response direction configured
    When the detected axis movement is opposite to the expected direction
    Then the reversal detector SHALL flag the axis as potentially reversed

  @AC-557.2
  Scenario: Detection result is surfaced as a diagnostic warning
    Given a potentially reversed axis has been detected
    When the diagnostic subsystem processes the detection result
    Then a reversal warning SHALL be emitted on the diagnostic channel

  @AC-557.3
  Scenario: User can confirm reversal and apply invert flag
    Given a reversal warning has been issued for an axis
    When the user confirms the reversal via CLI
    Then the service SHALL apply the invert flag to that axis in the active profile

  @AC-557.4
  Scenario: Reversal state is persisted per device
    Given the user has confirmed a reversal for a specific device axis
    When the service restarts and the device reconnects
    Then the invert flag SHALL be restored from persistent storage for that device axis
