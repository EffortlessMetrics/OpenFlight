Feature: Service License Management
  As a flight simulation enthusiast
  I want service license management
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: License validity is verified at startup for premium feature access
    Given the system is configured for service license management
    When the feature is exercised
    Then license validity is verified at startup for premium feature access

  Scenario: Expired or invalid licenses degrade gracefully to free-tier features
    Given the system is configured for service license management
    When the feature is exercised
    Then expired or invalid licenses degrade gracefully to free-tier features

  Scenario: License status is visible in service info and CLI output
    Given the system is configured for service license management
    When the feature is exercised
    Then license status is visible in service info and CLI output

  Scenario: License verification works offline using a cached validation token
    Given the system is configured for service license management
    When the feature is exercised
    Then license verification works offline using a cached validation token
