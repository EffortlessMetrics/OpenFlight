Feature: Service Config Hot-Reload
  As a flight simulation enthusiast
  I want service config hot-reload
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Configuration changes are applied without restarting the service
    Given the system is configured for service config hot-reload
    When the feature is exercised
    Then configuration changes are applied without restarting the service

  Scenario: Hot-reload validates new configuration before applying it
    Given the system is configured for service config hot-reload
    When the feature is exercised
    Then hot-reload validates new configuration before applying it

  Scenario: Failed hot-reload rolls back to the previous valid configuration
    Given the system is configured for service config hot-reload
    When the feature is exercised
    Then failed hot-reload rolls back to the previous valid configuration

  Scenario: A reload event is published on the event bus after success
    Given the system is configured for service config hot-reload
    When the feature is exercised
    Then a reload event is published on the event bus after success
