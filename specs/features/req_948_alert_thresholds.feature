Feature: Alert Thresholds
  As a flight simulation enthusiast
  I want alert thresholds
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Configurable alert thresholds trigger on metric value exceedance
    Given the system is configured for alert thresholds
    When the feature is exercised
    Then configurable alert thresholds trigger on metric value exceedance

  Scenario: Alerts support severity levels from info through critical
    Given the system is configured for alert thresholds
    When the feature is exercised
    Then alerts support severity levels from info through critical

  Scenario: Alert state changes are emitted as structured events on the bus
    Given the system is configured for alert thresholds
    When the feature is exercised
    Then alert state changes are emitted as structured events on the bus

  Scenario: Alert cooldown prevents repeated firing for sustained threshold breach
    Given the system is configured for alert thresholds
    When the feature is exercised
    Then alert cooldown prevents repeated firing for sustained threshold breach
