@REQ-496 @product
Feature: Service Health Dashboard — Terminal Live Health Display  @AC-496.1
  Scenario: flightctl dashboard shows live axis values, adapter status, and metrics
    Given the service is running
    When `flightctl dashboard` is executed
    Then the terminal SHALL display live axis values, adapter connection status, and key metrics  @AC-496.2
  Scenario: Dashboard refreshes at configurable rate
    Given `flightctl dashboard` is running with a custom refresh_rate option
    When the configured interval elapses
    Then the dashboard SHALL refresh its displayed values at that rate  @AC-496.3
  Scenario: Dashboard can be launched in both color and monochrome modes
    Given `flightctl dashboard` is invoked with --no-color
    When the dashboard renders
    Then it SHALL display in monochrome without ANSI color codes  @AC-496.4
  Scenario: Dashboard exits cleanly on Ctrl+C
    Given `flightctl dashboard` is running
    When the user sends SIGINT (Ctrl+C)
    Then the dashboard SHALL restore the terminal state and exit with code 0
