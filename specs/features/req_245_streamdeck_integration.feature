@REQ-245 @product
Feature: Elgato Stream Deck buttons and display integrate with OpenFlight panel engine  @AC-245.1
  Scenario: Stream Deck connected via USB HID is detected automatically
    Given no Stream Deck device is present
    When a Stream Deck is connected via USB
    Then the flight-streamdeck driver SHALL detect the device and register it with the panel engine without manual intervention  @AC-245.2
  Scenario: Button press events routed to panel engine rules
    Given a Stream Deck is registered and a rule is bound to button index 0
    When button 0 is pressed
    Then a panel engine input event SHALL be dispatched and the matching rule SHALL be evaluated  @AC-245.3
  Scenario: Key images updated via panel engine LED display state machine
    Given a Stream Deck key bound to a panel state that transitions from OFF to ON
    When the panel engine state machine transitions the state to ON
    Then the corresponding Stream Deck key image SHALL be updated to reflect the ON state  @AC-245.4
  Scenario: Stream Deck Mini MK2 and XL are all supported
    Given each of the Stream Deck Mini, MK2, and XL models connected in turn
    When the driver enumerates HID devices
    Then each model SHALL be identified by its USB product ID and operated without error  @AC-245.5
  Scenario: Profile maps Stream Deck keys to named actions
    Given a profile containing a key_map binding key 3 to action landing_lights_toggle
    When the profile is applied and key 3 is pressed
    Then the action landing_lights_toggle SHALL be emitted to the panel engine rules pipeline  @AC-245.6
  Scenario: Disconnect and reconnect handled without service restart
    Given a Stream Deck currently registered with the panel engine
    When the device is unplugged and then replugged
    Then the driver SHALL cleanly remove and re-register the device and resume normal operation without restarting the service
