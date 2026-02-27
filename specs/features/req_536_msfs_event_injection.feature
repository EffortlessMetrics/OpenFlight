Feature: MSFS SimConnect Event Injection
  As a flight simulation enthusiast
  I want button inputs mapped to named SimConnect events
  So that cockpit actions can be triggered from my hardware controls

  Background:
    Given the OpenFlight service is running
    And a SimConnect connection to MSFS is established

  Scenario: Named SimConnect event mapped to button press
    Given a profile rule mapping button 5 to SimConnect event "TOGGLE_MASTER_BATTERY"
    When button 5 is pressed
    Then the SimConnect event "TOGGLE_MASTER_BATTERY" is transmitted to MSFS

  Scenario: Rapid button presses are rate-limited to sim frame rate
    Given a profile rule mapping button 3 to SimConnect event "FLAPS_UP"
    When button 3 is pressed 10 times within 100 milliseconds
    Then the number of transmitted events does not exceed the sim frame rate limit

  Scenario: Failed event injection logged with error code
    Given the SimConnect connection becomes unavailable
    When a mapped button is pressed
    Then the failure is logged with the SimConnect error code
    And the log entry includes the event name and timestamp
