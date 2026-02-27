@REQ-389 @infra
Feature: Multi-Instance Prevention via Platform File Lock  @AC-389.1
  Scenario: Service acquires a platform file lock on startup
    Given the service is starting for the first time on the platform
    When startup completes successfully
    Then it SHALL hold a platform file lock for the duration of its run  @AC-389.2
  Scenario: A second service instance exits with a clear error message
    Given a service instance is already running and holding the file lock
    When a second service instance attempts to start
    Then the second instance SHALL exit immediately with a clear error message  @AC-389.3
  Scenario: Lock is released automatically when the service exits or crashes
    Given the service holds the file lock
    When the service exits normally or terminates abnormally
    Then the file lock SHALL be released automatically by the OS  @AC-389.4
  Scenario: Lock file path is documented and configurable
    Given the service default configuration
    When the lock file path setting is inspected
    Then it SHALL have a documented default path and be overridable via configuration  @AC-389.5
  Scenario: flightctl status shows whether the service is running
    Given a service instance is running and holds the lock
    When the user runs flightctl status
    Then the command SHALL report the service as running  @AC-389.6
  Scenario: Multi-instance check is verified by an integration test with two instances
    Given an integration test that launches two service instances sequentially
    When the second instance starts while the first is still running
    Then the second instance SHALL fail to start with a non-zero exit code and an error message
