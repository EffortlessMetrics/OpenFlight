Feature: Axis Input Debounce
  As a flight simulation enthusiast
  I want the axis engine to support button debounce for digital inputs
  So that noisy button contacts do not produce unintended repeated events

  Background:
    Given the OpenFlight service is running

  Scenario: Button debounce window is configurable in milliseconds
    Given a button debounce window is set in profile
    When the profile is loaded
    Then the debounce window is applied to the configured button

  Scenario: Rapid toggles within debounce window are treated as single press
    Given a button debounce window of 20ms is active
    When the button toggles 5 times within 20 milliseconds
    Then only a single press event is registered

  Scenario: Debounce applies independently per button
    Given two buttons have different debounce windows configured
    When both buttons are pressed simultaneously
    Then each button uses its own independent debounce window

  Scenario: Debounce state is not counted toward rate limiting
    Given rate limiting and debounce are both enabled for a button
    When a debounced press event is emitted
    Then the debounce does not consume rate limit budget
