Feature: Service API Rate Limiting
  As a flight simulation enthusiast
  I want service api rate limiting
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: IPC endpoints enforce configurable request rate limits
    Given the system is configured for service api rate limiting
    When the feature is exercised
    Then iPC endpoints enforce configurable request rate limits

  Scenario: Rate-limited requests receive a descriptive rejection response
    Given the system is configured for service api rate limiting
    When the feature is exercised
    Then rate-limited requests receive a descriptive rejection response

  Scenario: Rate limit counters reset on a sliding window basis
    Given the system is configured for service api rate limiting
    When the feature is exercised
    Then rate limit counters reset on a sliding window basis

  Scenario: Critical control endpoints are exempt from rate limiting
    Given the system is configured for service api rate limiting
    When the feature is exercised
    Then critical control endpoints are exempt from rate limiting
