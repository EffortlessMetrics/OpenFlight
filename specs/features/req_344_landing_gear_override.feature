@REQ-344 @product
Feature: Landing Gear Override  @AC-344.1
  Scenario: Button mapping triggers in-game gear cycle action
    Given a button is mapped to the gear cycle action in the active profile
    When the user presses that button
    Then the service SHALL send the gear cycle command to the simulator  @AC-344.2
  Scenario: Gear state is tracked via sim telemetry
    Given the service is connected to the simulator
    When the simulator reports a gear state change (up, down, or in-transit)
    Then the service SHALL update its internal gear state accordingly  @AC-344.3
  Scenario: Gear state drives an axis indicator
    Given the simulator reports gear fully up
    When the axis indicator for gear state is read
    Then the indicator SHALL output 0.0 for up and 1.0 for fully down  @AC-344.4
  Scenario: Gear transition plays a haptic/FFB cue if configured
    Given the active profile has a haptic cue configured for gear transitions
    When the gear transitions between states
    Then the service SHALL trigger the configured haptic/FFB cue  @AC-344.5
  Scenario: Gear toggle is blocked during overspeed
    Given the aircraft is in an overspeed condition
    When the user attempts to toggle the landing gear
    Then the service SHALL block the gear toggle command and log the inhibit reason  @AC-344.6
  Scenario: Gear state is shown in CLI diagnostics
    Given the service is running and gear state is known
    When the user runs "flightctl diagnostics"
    Then the output SHALL include the current landing gear state
