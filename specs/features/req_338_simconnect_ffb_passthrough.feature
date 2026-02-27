@REQ-338 @product
Feature: SimConnect FFB Passthrough  @AC-338.1
  Scenario: Service relays SimConnect FFB forces to HID FFB device
    Given a SimConnect session providing FFB force data and a HID FFB device connected
    When the passthrough mode is active
    Then the service SHALL forward the SimConnect FFB force commands to the HID device  @AC-338.2
  Scenario: Force translation preserves stick shake and G-force effects
    Given SimConnect reports a stick-shake event and a sustained G-force effect
    When the effects are translated for the HID device
    Then the HID device SHALL receive distinct stick-shake and G-force effect commands with their original magnitudes preserved  @AC-338.3
  Scenario: Multiple simultaneous FFB effects are blended correctly
    Given SimConnect sends a stick-shake effect and a trim-spring effect simultaneously
    When the FFB engine processes both effects
    Then the output sent to the HID device SHALL be the correctly blended combination of both effects  @AC-338.4
  Scenario: FFB passthrough is controllable per profile
    Given one aircraft profile has FFB passthrough enabled and another has it disabled
    When each profile is loaded
    Then FFB passthrough SHALL be active only for the profile that enables it  @AC-338.5
  Scenario: FFB passthrough respects the safety envelope
    Given the safety envelope defines a maximum force limit
    When SimConnect sends an FFB command that exceeds the maximum force limit
    Then the service SHALL clamp the force to the configured limit before forwarding it to the HID device  @AC-338.6
  Scenario: FFB force values are logged at debug level
    Given FFB passthrough is active and processing force commands
    When the service logs FFB force values
    Then the log entries for FFB force values SHALL be emitted at debug level and not at info level
