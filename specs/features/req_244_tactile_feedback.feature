@REQ-244 @product
Feature: Tactile transducers receive FFB haptic events from OpenFlight  @AC-244.1
  Scenario: Tactile device accepts audio-format haptic waveforms
    Given a tactile transducer device registered with the flight-tactile driver
    When a haptic waveform in audio PCM format is submitted
    Then the device SHALL render the waveform without error  @AC-244.2
  Scenario: Stall buffet gear rumble and landing thud effects are implemented
    Given the sim reports a stall, a gear-down event, and a touchdown event in sequence
    When the tactile effect engine processes each event
    Then the stall buffet, gear rumble, and landing thud waveforms SHALL each be dispatched to the tactile device  @AC-244.3
  Scenario: Effect intensity scales with sim parameter
    Given a stall buffet effect configured to scale with G-force
    When the simulator reports increasing G-force values
    Then the rendered waveform amplitude SHALL increase proportionally to the G-force value  @AC-244.4
  Scenario: Tactile output does not affect RT spine timing
    Given the tactile driver is actively rendering effects
    When the RT spine executes its 250 Hz tick
    Then the tick completion time SHALL remain within the p99 jitter budget of 0.5 ms  @AC-244.5
  Scenario: Tactile effects are configurable per aircraft type in profile
    Given two aircraft profiles with different tactile effect mappings loaded
    When the active aircraft profile is switched
    Then the tactile engine SHALL apply the effect configuration from the newly active aircraft profile  @AC-244.6
  Scenario: Multiple tactile devices driven simultaneously
    Given a seat transducer and a pedal transducer both registered
    When a landing thud effect is triggered
    Then both the seat and pedal devices SHALL receive and render the effect concurrently
