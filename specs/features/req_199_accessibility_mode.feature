@REQ-199 @product
Feature: Accessibility mode enables simplified controls for users with disabilities  @AC-199.1
  Scenario: Accessibility mode reduces axis sensitivity for tremor compensation
    Given accessibility mode is enabled
    When axis input is received with small rapid fluctuations
    Then axis sensitivity SHALL be reduced to compensate for tremor  @AC-199.2
  Scenario: Button hold time configurable between 0.1s and 2.0s
    Given accessibility mode is enabled
    When a button hold time of 0.5 seconds is configured
    Then button presses held for less than the configured time SHALL NOT register  @AC-199.3
  Scenario: Axis assistance filter smooths jitter without adding latency
    Given accessibility mode is enabled with jitter smoothing active
    When rapid axis jitter is received
    Then the output SHALL be smoothed without introducing perceptible latency  @AC-199.4
  Scenario: Single-axis fly-by-wire mode allows one-hand control
    Given accessibility mode is enabled with single-axis fly-by-wire active
    When only one axis is being controlled
    Then the flight model assistance SHALL compensate to maintain stable flight with one-hand input  @AC-199.5
  Scenario: Accessibility settings stored in user profile not device profile
    Given accessibility settings have been configured
    When the user profile is exported
    Then accessibility settings SHALL be present in the user profile and absent from the device profile  @AC-199.6
  Scenario: Accessibility mode toggled without service restart
    Given accessibility mode is currently disabled
    When the user enables accessibility mode via CLI or UI
    Then accessibility mode SHALL activate without requiring a service restart
