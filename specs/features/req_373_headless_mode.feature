@REQ-373 @product
Feature: Headless Mode for Embedded Deployments — Run Without UI/IPC

  @AC-373.1
  Scenario: Service starts in headless mode with --headless flag
    Given a system configured for embedded deployment
    When `flightd --headless` is invoked
    Then the service SHALL start with IPC and UI subsystems disabled

  @AC-373.2
  Scenario: Headless mode processes axis data at full RT rate
    Given the service running in headless mode
    When the RT spine is active
    Then all axis data SHALL be processed at the full 250 Hz RT rate

  @AC-373.3
  Scenario: Headless mode logs to stderr only
    Given the service running in headless mode
    When log output is produced
    Then it SHALL be written to stderr only with no file rotation enabled

  @AC-373.4
  Scenario: flightctl returns clear error when service is in headless mode
    Given the service running in headless mode
    When a user attempts to connect with `flightctl`
    Then `flightctl` SHALL return a clear error message indicating the service is in headless mode

  @AC-373.5
  Scenario: Headless mode reduces startup time by at least 200 ms
    Given the service measured startup time in normal mode
    When the service starts in headless mode (no IPC socket setup)
    Then the startup time SHALL be at least 200 ms less than normal mode startup

  @AC-373.6
  Scenario: Config file path is respected in headless mode
    Given a config file at a specified path
    When the service starts with `--headless --config <path>`
    Then the service SHALL load configuration from the specified path
