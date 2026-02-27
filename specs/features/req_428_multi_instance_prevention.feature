@REQ-428 @product
Feature: Multi-Instance Prevention — Prevent Multiple Service Instances Running Simultaneously

  @AC-428.1
  Scenario: Service acquires exclusive file lock on startup
    Given no other service instance is running
    When the service starts
    Then it SHALL acquire an exclusive file lock before accepting connections

  @AC-428.2
  Scenario: Second service instance fails to start with clear error message
    Given a service instance is already running and holding the lock
    When a second instance attempts to start
    Then the second instance SHALL exit with a non-zero code and print a clear error message

  @AC-428.3
  Scenario: Lock is released on clean shutdown
    Given a running service instance holding the file lock
    When the service receives a graceful shutdown signal
    Then the file lock SHALL be released before the process exits

  @AC-428.4
  Scenario: Stale lock from crashed instance is detected and cleared on restart
    Given a stale lock file left by a previously crashed instance
    When a new service instance starts
    Then it SHALL detect the stale lock, clear it, and start successfully
