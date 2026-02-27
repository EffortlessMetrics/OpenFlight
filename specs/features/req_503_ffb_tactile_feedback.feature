@REQ-503 @product
Feature: FFB Tactile Feedback Mode — Rumble Patterns Without Directional Force  @AC-503.1
  Scenario: Tactile mode generates rumble patterns without directional force
    Given the FFB engine is in tactile mode
    When a tactile feedback effect is triggered
    Then the output SHALL produce periodic rumble with zero net directional force component  @AC-503.2
  Scenario: Tactile intensity and frequency are configurable
    Given a tactile effect profile with intensity 0.5 and frequency 40 Hz
    When the effect is applied
    Then the FFB engine SHALL output rumble at the specified intensity and frequency  @AC-503.3
  Scenario: Tactile can be triggered by sim events
    Given the service is connected to a simulator
    When an engine-start event is received from the simulator
    Then the configured tactile effect for engine-start SHALL be activated  @AC-503.4
  Scenario: Tactile mode works on both FFB joysticks and tactile transducers
    Given a mixed device setup with one FFB joystick and one tactile transducer
    When a tactile feedback event is triggered
    Then both devices SHALL receive appropriate tactile output for their device type
