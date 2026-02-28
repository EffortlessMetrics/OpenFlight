Feature: Structured Logging
  As a flight simulation enthusiast
  I want structured logging
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Log output is formatted as structured JSON with consistent field schema
    Given the system is configured for structured logging
    When the feature is exercised
    Then log output is formatted as structured JSON with consistent field schema

  Scenario: Each log entry includes timestamp, level, component, and message fields
    Given the system is configured for structured logging
    When the feature is exercised
    Then each log entry includes timestamp, level, component, and message fields

  Scenario: Structured logs include trace and span IDs for correlation
    Given the system is configured for structured logging
    When the feature is exercised
    Then structured logs include trace and span IDs for correlation

  Scenario: Log format is configurable between JSON and human-readable output
    Given the system is configured for structured logging
    When the feature is exercised
    Then log format is configurable between JSON and human-readable output
