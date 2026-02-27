Feature: Chaos Testing
  As a flight simulation enthusiast
  I want chaos testing
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Fault injection simulates device disconnection during operation
    Given the system is configured for chaos testing
    When the feature is exercised
    Then fault injection simulates device disconnection during operation

  Scenario: Chaos tests verify graceful degradation under adapter failures
    Given the system is configured for chaos testing
    When the feature is exercised
    Then chaos tests verify graceful degradation under adapter failures

  Scenario: Random delay injection tests RT spine jitter resilience
    Given the system is configured for chaos testing
    When the feature is exercised
    Then random delay injection tests RT spine jitter resilience

  Scenario: Chaos test results document recovery time for each failure mode
    Given the system is configured for chaos testing
    When the feature is exercised
    Then chaos test results document recovery time for each failure mode
