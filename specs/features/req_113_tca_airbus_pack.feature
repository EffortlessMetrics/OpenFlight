@REQ-113 @product
Feature: Thrustmaster TCA Officer Pack (Airbus) device support

  @AC-113.1
  Scenario: Sidestick X axis at full right deflection produces +1.0
    Given a TCA Officer Pack report with sidestick X raw at maximum
    When the report is parsed
    Then sidestick_x SHALL be within 0.001 of 1.0

  @AC-113.1
  Scenario: Sidestick Y axis at full forward deflection produces -1.0
    Given a TCA Officer Pack report with sidestick Y raw at minimum
    When the report is parsed
    Then sidestick_y SHALL be within 0.001 of -1.0

  @AC-113.1
  Scenario: Sidestick axes have a center detent that maps to 0.0
    Given a TCA Officer Pack report with sidestick X and Y raws at their mechanical center
    When the report is parsed
    Then sidestick_x SHALL be within 0.01 of 0.0
    And sidestick_y SHALL be within 0.01 of 0.0

  @AC-113.2
  Scenario: Throttle lever in IDLE gate is detected correctly
    Given a TCA Officer Pack report with throttle raw at the IDLE detent position
    When the throttle gate is evaluated
    Then the active gate SHALL be IDLE

  @AC-113.2
  Scenario: Throttle lever in TOGA gate is detected correctly
    Given a TCA Officer Pack report with throttle raw at the TOGA detent position
    When the throttle gate is evaluated
    Then the active gate SHALL be TOGA

  @AC-113.2
  Scenario: Throttle lever in CL gate is detected correctly
    Given a TCA Officer Pack report with throttle raw at the CL detent position
    When the throttle gate is evaluated
    Then the active gate SHALL be CL

  @AC-113.2
  Scenario: Throttle lever in FLX/MCT gate is detected correctly
    Given a TCA Officer Pack report with throttle raw at the FLX_MCT detent position
    When the throttle gate is evaluated
    Then the active gate SHALL be FLX_MCT

  @AC-113.3
  Scenario: Spoiler lever full forward produces 0.0 output
    Given a TCA Officer Pack report with spoiler lever raw at minimum
    When the report is parsed
    Then spoiler_lever SHALL be within 0.001 of 0.0

  @AC-113.4
  Scenario: Each button state is independently reported
    Given a TCA Officer Pack report with exactly one button bit set
    When the report is parsed
    Then only that button SHALL be reported as pressed

  @AC-113.5
  Scenario: TCA Officer Pack has no force feedback output
    Given a TCA Officer Pack device instance
    When FFB capability is queried
    Then has_ffb SHALL be false
