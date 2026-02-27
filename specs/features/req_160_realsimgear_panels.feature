@REQ-160 @product
Feature: RealSimGear avionics panels

  @AC-160.1
  Scenario: G1000 PFD soft key press decoded
    Given a RealSimGear G1000 PFD panel is connected and bound
    When a soft key is pressed
    Then the corresponding soft key event SHALL be emitted with the correct key index

  @AC-160.2
  Scenario: GNS 530 outer knob step decoded
    Given a RealSimGear GNS 530 panel is connected and bound
    When the outer knob is rotated one step
    Then a knob step event SHALL be emitted with the correct direction and magnitude

  @AC-160.3
  Scenario: GNS 430W knob CW produces increment event
    Given a RealSimGear GNS 430W panel is connected and bound
    When the main knob is rotated one step clockwise
    Then an increment event SHALL be emitted for that knob

  @AC-160.4
  Scenario: KAP 140 AP button press decoded
    Given a RealSimGear KAP 140 panel is connected and bound
    When an autopilot button is pressed
    Then the corresponding autopilot button event SHALL be emitted

  @AC-160.5
  Scenario: GFC 600 Flight Director mode change
    Given a RealSimGear GFC 600 panel is connected and bound
    When a flight director mode button is pressed
    Then the mode change event SHALL be emitted with the correct mode identifier

  @AC-160.6
  Scenario: Panel identified by serial number
    Given a RealSimGear panel with a unique serial number is connected
    When the panel is enumerated
    Then the panel SHALL be identified by its serial number for profile binding

  @AC-160.7
  Scenario: All keys cleared on profile unload
    Given a RealSimGear panel is bound to an active profile
    When the profile is unloaded
    Then all key bindings SHALL be cleared and the panel SHALL return to its default state
