@REQ-494 @product
Feature: Auto-Update Check — Periodic Update Channel Polling  @AC-494.1
  Scenario: Service checks configured update channel on startup
    Given a service config specifying an update channel URL
    When the service starts
    Then it SHALL query the update channel for available releases  @AC-494.2
  Scenario: Available updates are logged and reported in flightctl status
    Given an update is available on the configured channel
    When `flightctl status` is executed
    Then the output SHALL indicate that an update is available and log an informational message  @AC-494.3
  Scenario: Automatic update download is opt-in and disabled by default
    Given the service config does not explicitly enable automatic downloads
    When an update is detected
    Then the service SHALL NOT automatically download or install the update  @AC-494.4
  Scenario: Update check interval is configurable
    Given a service config specifying a custom update_check_interval
    When the service is running
    Then it SHALL respect the configured interval between update checks
