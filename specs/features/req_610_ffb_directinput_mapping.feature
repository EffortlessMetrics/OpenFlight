Feature: FFB DirectInput Force Mapping
  As a flight simulation enthusiast
  I want flight physics to be mapped to DirectInput FFB effects
  So that I experience realistic force feedback on my joystick

  Background:
    Given the OpenFlight service is running
    And a DirectInput-compatible FFB joystick is connected

  Scenario: G-force is mapped to Spring and Damper effects
    When the simulator reports a G-force value
    Then the FFB engine applies Spring and Damper DirectInput effects proportionally

  Scenario: Turbulence maps to Periodic Sine effects
    When the simulator reports turbulence data
    Then the FFB engine applies Periodic Sine DirectInput effects

  Scenario: Stall warning maps to Periodic Random effects
    When the simulator reports a stall warning condition
    Then the FFB engine applies Periodic Random DirectInput effects

  Scenario: Effect intensity scales with configurable gain setting
    Given the FFB gain is set to 75%
    When any FFB effect is applied
    Then the effect intensity is scaled to 75% of its maximum value
