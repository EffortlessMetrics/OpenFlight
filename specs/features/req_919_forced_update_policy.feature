Feature: Forced Update Policy
  As a flight simulation enthusiast
  I want forced update policy
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Minimum version enforcement prevents running versions below policy threshold
    Given the system is configured for forced update policy
    When the feature is exercised
    Then minimum version enforcement prevents running versions below policy threshold

  Scenario: Forced update displays clear message explaining why update is required
    Given the system is configured for forced update policy
    When the feature is exercised
    Then forced update displays clear message explaining why update is required

  Scenario: Service enters degraded mode with limited functionality until update completes
    Given the system is configured for forced update policy
    When the feature is exercised
    Then service enters degraded mode with limited functionality until update completes

  Scenario: Forced update policy is signed and verified to prevent spoofing
    Given the system is configured for forced update policy
    When the feature is exercised
    Then forced update policy is signed and verified to prevent spoofing
