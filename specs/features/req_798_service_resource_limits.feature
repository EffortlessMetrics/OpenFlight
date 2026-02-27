Feature: Service Resource Limits
  As a flight simulation enthusiast
  I want service resource limits
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Configurable memory limits
    Given the system is configured for service resource limits
    When the feature is exercised
    Then service enforces configurable memory usage limits

  Scenario: Configurable CPU limits
    Given the system is configured for service resource limits
    When the feature is exercised
    Then service enforces configurable cpu usage limits

  Scenario: Warning on approaching limits
    Given the system is configured for service resource limits
    When the feature is exercised
    Then approaching resource limits triggers a warning

  Scenario: Graceful degradation on exceed
    Given the system is configured for service resource limits
    When the feature is exercised
    Then exceeding hard limits triggers graceful degradation
