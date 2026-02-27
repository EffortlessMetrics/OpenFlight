Feature: Service Watchdog Integration
  As a flight simulation enthusiast
  I want service watchdog integration
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Integrate with OS-level watchdog
    Given the system is configured for service watchdog integration
    When the feature is exercised
    Then service integrates with OS-level watchdog mechanisms (MMCSS/systemd)

  Scenario: Notify watchdog each cycle
    Given the system is configured for service watchdog integration
    When the feature is exercised
    Then watchdog is notified on each successful processing cycle

  Scenario: Auto-restart on missed notifications
    Given the system is configured for service watchdog integration
    When the feature is exercised
    Then missed watchdog notifications trigger automatic service restart

  Scenario: Configurable watchdog timeout
    Given the system is configured for service watchdog integration
    When the feature is exercised
    Then watchdog timeout is configurable in service settings
