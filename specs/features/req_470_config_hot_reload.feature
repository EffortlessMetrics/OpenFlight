@REQ-470 @product
Feature: Config Hot-Reload Without Restart — Atomic Config File Reload  @AC-470.1
  Scenario: Service detects config file changes within 2 seconds
    Given the service is running with a watched config file
    When the config file is modified on disk
    Then the service SHALL detect the change within 2 seconds  @AC-470.2
  Scenario: Valid config changes are applied atomically
    Given the service detects a valid config file change
    When the new config is loaded
    Then all config values SHALL be swapped atomically with no partial state visible  @AC-470.3
  Scenario: Invalid config changes are rejected and original is retained
    Given the service is running with a valid config
    When a config file with a syntax error is written
    Then the service SHALL reject the invalid config, log an error, and continue using the original  @AC-470.4
  Scenario: Hot-reload events are broadcast on the flight-bus
    Given the service is running with hot-reload enabled
    When a valid config reload occurs
    Then a config-reloaded event SHALL be broadcast on the flight-bus
