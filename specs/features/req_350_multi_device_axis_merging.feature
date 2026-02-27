@REQ-350 @axis @multi-device @merging
Feature: Multi-device axis merging
  As a user with multiple input devices
  I want to merge physical axes onto a single logical axis
  So that split-axis throttles and dual-stick setups work correctly

  Scenario: Two physical axes map to one logical axis  @AC-350.1
    Given device A has axis 0 and device B has axis 0
    When both are mapped to logical axis "throttle"
    Then the logical axis SHALL receive input from both devices

  Scenario: Merge mode is configurable  @AC-350.2
    Given a logical axis is configured with merge mode "priority"
    When the merge mode is changed to "sum"
    Then the new merge mode SHALL take effect on the next tick

  Scenario: Priority merge selects highest absolute value  @AC-350.3
    Given device A reports +0.3 and device B reports -0.7 on their respective axes
    When both axes are merged with mode "priority"
    Then the logical axis output SHALL be -0.7

  Scenario: Sum merge clamps to valid range  @AC-350.4
    Given device A reports +0.8 and device B reports +0.6 on their respective axes
    When both axes are merged with mode "sum"
    Then the logical axis output SHALL be clamped to 1.0

  Scenario: Merged axis survives device disconnection  @AC-350.5
    Given two devices are merged onto logical axis "rudder"
    When one device is disconnected
    Then the logical axis SHALL continue to report values from the remaining device

  Scenario: Each merge source has independent deadzone  @AC-350.6
    Given device A source has deadzone 0.05 and device B source has deadzone 0.10
    When device A reports 0.04 and device B reports 0.09
    Then device A source output SHALL be zero
    And device B source output SHALL be zero
    And device A reporting 0.06 SHALL produce non-zero output
