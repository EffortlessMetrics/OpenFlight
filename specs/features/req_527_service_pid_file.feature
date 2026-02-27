@REQ-527 @product
Feature: Service PID File — Runtime PID Tracking for flightd  @AC-527.1
  Scenario: Service writes PID file on Linux startup
    Given the flightd service is starting on a Linux host
    When startup completes successfully
    Then a PID file SHALL exist at /run/user/<UID>/openflight.pid containing the process ID  @AC-527.2
  Scenario: Service writes PID file on Windows startup
    Given the flightd service is starting on a Windows host
    When startup completes successfully
    Then a PID file SHALL exist at %TEMP%\openflight.pid containing the process ID  @AC-527.3
  Scenario: PID file is removed on clean shutdown
    Given flightd is running and its PID file exists
    When the service receives a shutdown signal and exits cleanly
    Then the PID file SHALL no longer exist on the filesystem  @AC-527.4
  Scenario: flightctl status uses PID file to detect running service
    Given the flightd PID file exists with a valid PID
    When `flightctl status` is executed
    Then the output SHALL indicate the service is running with the PID from the file
