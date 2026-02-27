@REQ-531 @product
Feature: Sim Variable Rate Override — Per-Variable Update Rate Configuration  @AC-531.1
  Scenario: Per-variable update rate can be set in profile
    Given a profile that sets INDICATED_AIRSPEED update_rate_hz = 10
    When the SimConnect adapter initialises subscriptions
    Then INDICATED_AIRSPEED SHALL be polled at 10 Hz  @AC-531.2
  Scenario: High-frequency variables run at sim frame rate
    Given a variable configured with update_rate = frame
    When the sim is running at 60 fps
    Then the adapter SHALL deliver updates for that variable at up to 60 Hz  @AC-531.3
  Scenario: Low-frequency variables use reduced polling
    Given a variable configured with update_rate_hz = 1
    When the sim is running
    Then the adapter SHALL poll that variable no more than once per second  @AC-531.4
  Scenario: Rate configuration is applied at adapter initialisation
    Given a profile with per-variable rate overrides is loaded
    When the simulator adapter is initialised
    Then all variable subscription rates SHALL reflect the profile configuration
