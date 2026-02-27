Feature: Update Scheduling
  As a flight simulation enthusiast
  I want update scheduling
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Updates can be deferred to next service restart via user preference
    Given the system is configured for update scheduling
    When the feature is exercised
    Then updates can be deferred to next service restart via user preference

  Scenario: Scheduled updates apply automatically when service starts with pending update
    Given the system is configured for update scheduling
    When the feature is exercised
    Then scheduled updates apply automatically when service starts with pending update

  Scenario: Update schedule is configurable with time-of-day preference
    Given the system is configured for update scheduling
    When the feature is exercised
    Then update schedule is configurable with time-of-day preference

  Scenario: Deferred update notification persists until update is applied or dismissed
    Given the system is configured for update scheduling
    When the feature is exercised
    Then deferred update notification persists until update is applied or dismissed
