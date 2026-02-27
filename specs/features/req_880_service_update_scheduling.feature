Feature: Service Update Scheduling
  As a flight simulation enthusiast
  I want service update scheduling
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Updates can be scheduled for a user-specified time window
    Given the system is configured for service update scheduling
    When the feature is exercised
    Then updates can be scheduled for a user-specified time window

  Scenario: Scheduled updates are skipped if a flight session is active
    Given the system is configured for service update scheduling
    When the feature is exercised
    Then scheduled updates are skipped if a flight session is active

  Scenario: Update schedule is persisted and survives service restarts
    Given the system is configured for service update scheduling
    When the feature is exercised
    Then update schedule is persisted and survives service restarts

  Scenario: Users receive a notification before a scheduled update begins
    Given the system is configured for service update scheduling
    When the feature is exercised
    Then users receive a notification before a scheduled update begins
