@REQ-319 @product
Feature: Input Combination Logic  @AC-319.1
  Scenario: Two axis inputs are combined with a configurable blend percentage
    Given two axis inputs are configured for combination with a blend of 60%
    When both axes produce input values
    Then the combined axis SHALL output a value blended at 60% between the two inputs  @AC-319.2
  Scenario: Sum and differential combination modes are both supported
    Given a combined axis configured in sum mode and another in differential mode
    When input values are applied to both axes
    Then the sum axis SHALL add the two inputs and the differential axis SHALL subtract them  @AC-319.3
  Scenario: Combined axis value is properly normalized after blend
    Given two axis inputs at their maximum values combined with 50% blend
    When the combined value is computed
    Then the output SHALL be normalized to the valid axis range [-1.0, 1.0]  @AC-319.4
  Scenario: Combination config is per-profile and per-axis
    Given two different aircraft profiles each with distinct combination settings for the same axis
    When profiles are switched
    Then the combination mode and blend SHALL reflect the active profile's configuration  @AC-319.5
  Scenario: Zero blend produces passthrough of primary axis
    Given a combined axis configured with a blend of 0%
    When the primary axis produces a value
    Then the combined axis output SHALL equal the primary axis value exactly  @AC-319.6
  Scenario: Combined axis is listed as virtual in device enumeration
    Given a combined axis has been created in the active profile
    When the user runs flightctl devices --list
    Then the combined axis SHALL appear in the output marked as a virtual axis
