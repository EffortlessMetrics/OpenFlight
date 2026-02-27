@REQ-573 @product
Feature: Axis Deadzone Hysteresis — Axis deadzone should support hysteresis to prevent chatter  @AC-573.1
  Scenario: Hysteresis prevents rapid on/off toggling at deadzone boundary
    Given an axis with a deadzone and hysteresis configured
    When the axis value oscillates rapidly at the deadzone boundary
    Then the output SHALL not toggle on and off rapidly between active and inactive states  @AC-573.2
  Scenario: Hysteresis width is configurable separately from deadzone
    Given an axis configuration with deadzone set to 0.05 and hysteresis set to 0.02
    When the configuration is loaded
    Then the deadzone and hysteresis SHALL be applied as independent parameters  @AC-573.3
  Scenario: Hysteresis is disabled when deadzone is zero
    Given an axis configuration with deadzone set to zero
    When the axis processes input
    Then hysteresis SHALL not be applied regardless of the hysteresis configuration value  @AC-573.4
  Scenario: Hysteresis state is tracked per axis instance
    Given two axis instances with the same deadzone and hysteresis config
    When each axis is at a different position relative to the deadzone boundary
    Then each axis SHALL maintain its own independent hysteresis state
