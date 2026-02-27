@REQ-574 @product
Feature: Service Update Check — Service should check for updates on startup and periodically  @AC-574.1
  Scenario: Update check queries release manifest from configured URL
    Given the update check URL is configured in the service config
    When the service performs an update check
    Then it SHALL fetch the release manifest from the configured URL  @AC-574.2
  Scenario: Available update version is logged on startup
    Given a newer version is available in the release manifest
    When the service starts up
    Then the available update version SHALL be logged at info level  @AC-574.3
  Scenario: Update check period is configurable
    Given the update check interval is set to 24 hours in config
    When the service is running
    Then update checks SHALL occur at the configured interval  @AC-574.4
  Scenario: Update check failures do not block service startup
    Given the update check URL is unreachable
    When the service starts
    Then the service SHALL complete startup successfully and log a warning about the failed update check
