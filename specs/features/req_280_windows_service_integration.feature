@REQ-280 @product
Feature: Windows service integration with MSI install, user context, recovery policy, and CLI control  @AC-280.1
  Scenario: flightd installs as a Windows service via MSI
    Given the OpenFlight MSI installer package
    When the user runs the installer to completion
    Then flightd SHALL be registered as a Windows service visible in the Services snap-in  @AC-280.2
  Scenario: Service starts automatically on user login
    Given flightd is installed as a Windows service with start type set to Automatic
    When the user logs into Windows
    Then the flightd service SHALL start without requiring manual intervention  @AC-280.3
  Scenario: Service stops cleanly on Windows shutdown
    Given the flightd service is running
    When Windows initiates a system shutdown sequence
    Then the service SHALL complete its shutdown handler and exit within the service control timeout  @AC-280.4
  Scenario: Service recovery policy restarts on crash
    Given the flightd service is configured with a recovery policy set to restart on first failure
    When the flightd process terminates unexpectedly
    Then the Windows service manager SHALL restart flightd automatically  @AC-280.5
  Scenario: Service runs in user context not SYSTEM
    Given flightd is installed and running as a Windows service
    When the service identity is queried via the service control manager
    Then the service SHALL be running under the installing user account and not the SYSTEM account  @AC-280.6
  Scenario: Service can be controlled via flightctl start stop status
    Given the flightd service is installed on Windows
    When the user runs flightctl with the start, stop, or status subcommand
    Then flightctl SHALL reflect the corresponding service state change or report the current service status
