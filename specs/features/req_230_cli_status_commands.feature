@REQ-230 @product
Feature: flightctl CLI provides rich status and inspection of all subsystems  @AC-230.1
  Scenario: flightctl status shows service health device count and active profile
    Given the OpenFlight service is running with devices connected
    When the user runs flightctl status
    Then the output SHALL include service health indicator, connected device count, and the name of the active profile  @AC-230.2
  Scenario: flightctl devices lists all connected devices with VID PID and status
    Given one or more HID devices are connected and managed by the service
    When the user runs flightctl devices
    Then each device SHALL be listed with its USB vendor ID, product ID, and connection status  @AC-230.3
  Scenario: flightctl axes shows live normalized axis values with update rate
    Given the service is processing axis inputs
    When the user runs flightctl axes
    Then the output SHALL show the current normalized value and update rate for each active axis  @AC-230.4
  Scenario: flightctl profile show dumps the active profile as TOML
    Given the service has an active profile loaded
    When the user runs flightctl profile show
    Then the complete active profile SHALL be printed to stdout in valid TOML format  @AC-230.5
  Scenario: JSON output mode available for tooling integration
    Given the OpenFlight service is running
    When the user runs flightctl --json status
    Then the output SHALL be well-formed JSON containing the same information as the human-readable status  @AC-230.6
  Scenario: All commands fail gracefully when service is not running
    Given the OpenFlight service is not running
    When the user runs any flightctl command
    Then the command SHALL exit with code 1 and print a clear human-readable message indicating the service is not running
