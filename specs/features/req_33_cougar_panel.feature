@REQ-33
Feature: Cougar MFD panel type detection, report building, and verification

  @AC-33.1
  Scenario: Cougar MFD type detected from product ID
    Given a Cougar MFD product ID
    When the type is detected from the product ID
    Then the correct MFD variant SHALL be returned

  @AC-33.1
  Scenario: Cougar MFD LED mapping is correct
    Given a Cougar MFD variant
    When the LED mapping is queried
    Then each logical LED SHALL map to the correct HID report position

  @AC-33.2
  Scenario: MFD HID report encodes LED state correctly
    Given a set of LED state values for a Cougar MFD
    When a HID report is built
    Then the report bytes SHALL encode the LED state correctly

  @AC-33.2
  Scenario: LED brightness values are clamped to valid range
    Given an LED brightness value outside the valid range
    When the brightness is applied
    Then the value SHALL be clamped to the maximum allowed brightness

  @AC-33.3
  Scenario: Verify test result latency analysis detects failures
    Given a verification test result with a latency value exceeding the requirement
    When the latency analysis is run
    Then the analysis SHALL report a latency violation

  @AC-33.3
  Scenario: Drift detection flags and initiates repair
    Given a Cougar MFD with detected LED drift
    When drift detection runs
    Then the drift SHALL be reported and a repair SHALL be initiated
