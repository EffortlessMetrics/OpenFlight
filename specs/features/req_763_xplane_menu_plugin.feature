Feature: X-Plane Menu Plugin
  As a flight simulation enthusiast
  I want x-plane menu plugin
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Add menu to plugins menu
    Given the system is configured for x-plane menu plugin
    When the feature is exercised
    Then x-plane plugin adds an openflight menu to the plugins menu

  Scenario: Enable and disable options
    Given the system is configured for x-plane menu plugin
    When the feature is exercised
    Then menu includes options to enable and disable the connection

  Scenario: Display connection status
    Given the system is configured for x-plane menu plugin
    When the feature is exercised
    Then menu displays current connection status

  Scenario: Persist menu state
    Given the system is configured for x-plane menu plugin
    When the feature is exercised
    Then menu state persists across x-plane sessions
