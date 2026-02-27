Feature: Service Diagnostic Bundle
  As a flight simulation enthusiast
  I want service diagnostic bundle
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Generate compressed diagnostic bundle
    Given the system is configured for service diagnostic bundle
    When the feature is exercised
    Then service generates a compressed diagnostic bundle for support analysis

  Scenario: Include logs, config, device info, metrics
    Given the system is configured for service diagnostic bundle
    When the feature is exercised
    Then bundle includes logs, configuration, device info, and system metrics

  Scenario: Redact sensitive data
    Given the system is configured for service diagnostic bundle
    When the feature is exercised
    Then sensitive data is redacted before inclusion in the bundle

  Scenario: Complete within 30 seconds
    Given the system is configured for service diagnostic bundle
    When the feature is exercised
    Then bundle generation completes within 30 seconds under normal conditions
