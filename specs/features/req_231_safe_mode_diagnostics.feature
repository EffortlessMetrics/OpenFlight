@REQ-231 @product
Feature: Safe mode generates diagnostic bundle and explanation on degradation  @AC-231.1
  Scenario: Safe mode activated on profile load failure
    Given a profile that contains a malformed or unloadable configuration
    When the service attempts to load the profile and encounters a failure
    Then the service SHALL activate safe mode and log the failure reason  @AC-231.2
  Scenario: Safe mode applies known-good axis defaults
    Given the service is running in safe mode
    When axis inputs are processed
    Then each axis SHALL use a deadzone of 3% and an expo of 20% as the known-good defaults  @AC-231.3
  Scenario: Safe mode generates a diagnostic bundle in temp directory
    Given safe mode has been activated due to a calibration error
    When the service initialises safe mode
    Then a diagnostic bundle file SHALL be written to the system temp directory  @AC-231.4
  Scenario: Diagnostic bundle contains failure reason last profile and device list
    Given a diagnostic bundle has been created
    When the bundle is inspected
    Then it SHALL contain the failure reason, a snapshot of the last known-good profile, and the list of connected devices  @AC-231.5
  Scenario: Service emits safe_mode_active event on bus
    Given the service transitions to safe mode
    When the transition completes
    Then the service SHALL emit a "safe_mode_active" event on the internal event bus  @AC-231.6
  Scenario: CLI shows safe mode status prominently in flightctl status
    Given the service is running in safe mode
    When the user runs flightctl status
    Then the output SHALL prominently indicate that the service is operating in safe mode
