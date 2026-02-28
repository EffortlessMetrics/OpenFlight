Feature: Service Backup Scheduler
  As a flight simulation enthusiast
  I want service backup scheduler
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Automatic configuration backup runs on a configurable schedule
    Given the system is configured for service backup scheduler
    When the feature is exercised
    Then automatic configuration backup runs on a configurable schedule

  Scenario: Backup includes profiles, device settings, and service configuration
    Given the system is configured for service backup scheduler
    When the feature is exercised
    Then backup includes profiles, device settings, and service configuration

  Scenario: Old backups are pruned based on retention policy
    Given the system is configured for service backup scheduler
    When the feature is exercised
    Then old backups are pruned based on retention policy

  Scenario: Backup status and history are queryable via the service API
    Given the system is configured for service backup scheduler
    When the feature is exercised
    Then backup status and history are queryable via the service API
