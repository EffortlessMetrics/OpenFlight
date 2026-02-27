Feature: IL-2 Axis Injection Support
  As a flight simulation enthusiast
  I want the IL-2 adapter to support axis injection via the IL-2 Export API
  So that processed axis values from OpenFlight can drive IL-2 controls

  Background:
    Given the OpenFlight service is running
    And the IL-2 export adapter is configured and connected

  Scenario: IL-2 export protocol supports axis value injection messages
    Given axis injection is enabled in the IL-2 adapter config
    When the axis engine outputs a normalized pitch value of 0.25
    Then the adapter sends an axis injection message for pitch with value 0.25 to IL-2

  Scenario: Injection is opt-in and disabled by default
    Given the IL-2 adapter config does not contain an "axis_injection" section
    When the adapter initialises
    Then no axis injection messages are sent to IL-2

  Scenario: Injection rate is capped at IL-2 expected frame rate
    Given the IL-2 expected frame rate cap is 60 Hz
    When the axis engine runs at 250 Hz
    Then the adapter sends at most 60 injection messages per second to IL-2

  Scenario: Injection errors are reported in adapter metrics
    Given the IL-2 export socket becomes unavailable
    When an axis injection message fails to send
    Then the adapter metric "il2_injection_errors_total" is incremented
