@REQ-1055 @product @user-journey
Feature: Safe mode user journey
  As a pilot relying on OpenFlight
  I want the system to enter safe mode on critical errors
  So that I retain basic flight control rather than losing all input

  @AC-1055.1
  Scenario: Enter safe mode when profile loading fails
    Given the OpenFlight service is running normally
    When a corrupt profile is loaded that causes a configuration error
    Then the service SHALL activate safe mode within one tick boundary
    And a "safe_mode_entered" event SHALL be emitted on the bus with the failure reason
    And the failure reason SHALL be logged at ERROR level
    And a diagnostic bundle SHALL be written to the system temp directory

  @AC-1055.2
  Scenario: Safe mode provides basic flight control with default profile
    Given the service has entered safe mode due to a configuration error
    When axis inputs are received from connected devices
    Then each axis SHALL be processed using the known-good default profile
    And the default profile SHALL apply a 3% deadzone and 20% expo curve
    And axis output SHALL continue at 250 Hz without interruption
    And the CLI status command SHALL display safe mode status prominently

  @AC-1055.3
  Scenario: Recover from safe mode by loading a valid profile
    Given the service is running in safe mode with the default profile active
    When the user loads a valid, well-formed profile via the CLI
    Then the service SHALL validate the new profile against the schema
    And the service SHALL exit safe mode and apply the new profile
    And a "safe_mode_exited" event SHALL be emitted on the bus
    And normal profile cascade processing SHALL resume
