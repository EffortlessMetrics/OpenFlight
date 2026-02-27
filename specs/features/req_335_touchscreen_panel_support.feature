@REQ-335 @product
Feature: Touchscreen Panel Support  @AC-335.1
  Scenario: Service sends button state to StreamDeck XL and similar touchscreens
    Given a StreamDeck XL is connected and configured
    When the service starts
    Then the service SHALL successfully send button state updates to the StreamDeck XL  @AC-335.2
  Scenario: Button labels are configurable per profile
    Given an aircraft profile defining button labels for panel keys
    When the profile is loaded
    Then the panel buttons SHALL display the labels specified in the profile  @AC-335.3
  Scenario: Button colors and icons reflect axis state
    Given a landing gear axis mapped to a panel button
    When the gear is in the up position
    Then the button SHALL display the configured green color or icon for the gear-up state  @AC-335.4
  Scenario: Panel updates are batched at a maximum of 30fps
    Given rapid axis state changes occurring faster than 30Hz
    When panel update messages are generated
    Then the service SHALL coalesce updates and send no more than 30 frames per second to the panel  @AC-335.5
  Scenario: Panel config is part of the aircraft profile
    Given a profile YAML for a specific aircraft
    When the profile contains a panels section
    Then the schema SHALL accept the panel configuration without error  @AC-335.6
  Scenario: Panel errors do not affect axis processing
    Given a StreamDeck XL that becomes unresponsive
    When a panel write fails
    Then the service SHALL log the error and continue axis processing at full 250Hz throughput
