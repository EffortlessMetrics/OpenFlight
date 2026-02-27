@REQ-134 @product
Feature: Gamepad flight control  @AC-134.1
  Scenario: Xbox Controller left stick maps to pitch and roll
    Given an Xbox Controller connected as a flight control device
    When the left thumbstick is deflected to its maximum right position
    Then the roll axis SHALL receive the fully-deflected positive value
    And the pitch axis SHALL reflect a centred value  @AC-134.2
  Scenario: Xbox Controller triggers map to throttle axis
    Given an Xbox Controller connected as a flight control device
    When the right trigger is fully depressed
    Then the throttle axis SHALL receive the maximum positive value  @AC-134.3
  Scenario: Button-to-axis assignment holds nose-up input
    Given a profile assigning the A button to a nose-up pitch increment
    When the A button is held continuously
    Then the pitch axis SHALL maintain a positive deflection for the duration of the hold  @AC-134.4
  Scenario: Deadzone applied to thumbstick axes
    Given a deadzone of 10 percent configured for the left thumbstick
    When the left thumbstick deflection is 8 percent of its maximum range
    Then the output pitch and roll axes SHALL both be zero  @AC-134.5
  Scenario: Gamepad axis curves configurable via profile
    Given a profile with an exponential curve applied to the left thumbstick roll axis
    When the thumbstick is deflected to 50 percent of its range
    Then the output roll axis SHALL reflect the exponentially scaled value not the raw linear value  @AC-134.6
  Scenario: Button repeat at configurable rate
    Given a profile with button repeat enabled at 10 Hz for the D-pad up button
    When the D-pad up button is held for 500 ms
    Then the bound action SHALL have fired approximately 5 times  @AC-134.7
  Scenario: Xbox Elite paddles assignable to custom axes
    Given an Xbox Elite Controller with rear paddles connected
    When a rear paddle is mapped to a custom flap axis in the profile
    Then pressing the paddle SHALL update the flap axis value accordingly  @AC-134.8
  Scenario: Disconnect and reconnect handled cleanly
    Given an Xbox Controller actively providing flight control input
    When the controller is disconnected and then reconnected
    Then the adapter SHALL detect the reconnection and resume axis output
    And no panic or stale axis values SHALL persist after reconnection
