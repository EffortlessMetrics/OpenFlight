@REQ-196 @product
Feature: OpenFlight writes sim variables based on panel/button events  @AC-196.1
  Scenario: Panel button event triggers SimConnect variable write
    Given a profile mapping a panel button to a SimConnect variable
    When the mapped button is pressed
    Then the corresponding SimConnect variable SHALL be written with the configured value  @AC-196.2
  Scenario: Write-back occurs within 50ms of button press
    Given a profile mapping a panel button to a SimConnect variable write
    When the button press event is detected
    Then the sim variable write SHALL complete within 50 milliseconds of the button press  @AC-196.3
  Scenario: Sim variable write failures are logged but do not crash service
    Given the SimConnect variable write channel is in a degraded state
    When a write-back is attempted
    Then the failure SHALL be logged and the service SHALL continue running  @AC-196.4
  Scenario: Write-back disabled when adapter is disconnected
    Given the SimConnect adapter is in a disconnected state
    When a button press triggers a write-back action
    Then the write-back SHALL be suppressed until the adapter reconnects  @AC-196.5
  Scenario: Per-variable write-back frequency is rate-limited
    Given a button mapped to a high-frequency toggle action
    When the button is pressed repeatedly within a short time window
    Then write-back attempts SHALL be limited to prevent overloading the simulator  @AC-196.6
  Scenario: Profile declares button-to-variable mappings declaratively
    Given a profile YAML with a button-to-variable mapping section
    When the profile is loaded
    Then each button SHALL be associated with a sim variable and write value defined declaratively in the profile
