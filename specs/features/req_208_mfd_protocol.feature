@REQ-208 @product
Feature: Multi-Function Display panels communicate via structured MFD protocol  @AC-208.1
  Scenario: MFD protocol supports page navigation soft-key and rotary encoder events
    Given an MFD panel connected via the MFD protocol
    When the panel emits page navigation, soft-key, and rotary encoder events
    Then all three event types SHALL be received and dispatched correctly  @AC-208.2
  Scenario: Display content updated via structured command from profile and rules engine
    Given a profile containing display content commands for an MFD
    When the rules engine evaluates the current state
    Then the MFD display SHALL be updated with the structured content command  @AC-208.3
  Scenario: MFD pages defined in profile as named states with transitions
    Given a profile defining MFD pages as named states with transition rules
    When a page navigation event is received
    Then the MFD SHALL transition to the correct named state as defined in the profile  @AC-208.4
  Scenario: MFD panel reports button press and encoder delta events
    Given an MFD panel with physical buttons and a rotary encoder
    When a button is pressed or an encoder is rotated
    Then the panel SHALL report a button press event or an encoder delta event respectively  @AC-208.5
  Scenario: Multiple MFD panels on same USB hub handled independently
    Given two MFD panels connected on the same USB hub
    When events arrive from each panel
    Then each panel's events SHALL be processed independently without cross-panel interference  @AC-208.6
  Scenario: MFD protocol version negotiated at connect time
    Given an MFD panel connecting to the service
    When the connection handshake occurs
    Then the protocol version SHALL be negotiated and logged before any events are processed
