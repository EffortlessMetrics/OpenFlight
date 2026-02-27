Feature: Privilege Separation
  As a flight simulation enthusiast
  I want privilege separation
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Service components run with minimal required OS permissions
    Given the system is configured for privilege separation
    When the feature is exercised
    Then service components run with minimal required OS permissions

  Scenario: HID access uses dedicated capability without full root or admin
    Given the system is configured for privilege separation
    When the feature is exercised
    Then hID access uses dedicated capability without full root or admin

  Scenario: Update application runs in separate privilege context from main service
    Given the system is configured for privilege separation
    When the feature is exercised
    Then update application runs in separate privilege context from main service

  Scenario: Plugin processes are sandboxed with reduced privilege set
    Given the system is configured for privilege separation
    When the feature is exercised
    Then plugin processes are sandboxed with reduced privilege set
