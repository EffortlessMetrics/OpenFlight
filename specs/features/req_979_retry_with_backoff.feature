Feature: Retry with Backoff
  As a flight simulation enthusiast
  I want retry with backoff
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Transient failures trigger automatic retry with exponential backoff
    Given the system is configured for retry with backoff
    When the feature is exercised
    Then transient failures trigger automatic retry with exponential backoff

  Scenario: Maximum retry count and backoff ceiling are configurable per operation
    Given the system is configured for retry with backoff
    When the feature is exercised
    Then maximum retry count and backoff ceiling are configurable per operation

  Scenario: Retry attempts are logged with attempt number and delay duration
    Given the system is configured for retry with backoff
    When the feature is exercised
    Then retry attempts are logged with attempt number and delay duration

  Scenario: Jitter is added to backoff delay to prevent thundering herd effects
    Given the system is configured for retry with backoff
    When the feature is exercised
    Then jitter is added to backoff delay to prevent thundering herd effects