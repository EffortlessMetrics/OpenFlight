Feature: Secure Defaults
  As a flight simulation enthusiast
  I want secure defaults
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Default configuration ships with IPC authentication enabled
    Given the system is configured for secure defaults
    When the feature is exercised
    Then default configuration ships with IPC authentication enabled

  Scenario: Default configuration disables remote access and binds to localhost only
    Given the system is configured for secure defaults
    When the feature is exercised
    Then default configuration disables remote access and binds to localhost only

  Scenario: Default plugin permissions follow least-privilege principle
    Given the system is configured for secure defaults
    When the feature is exercised
    Then default plugin permissions follow least-privilege principle

  Scenario: Default logging level excludes sensitive data from log output
    Given the system is configured for secure defaults
    When the feature is exercised
    Then default logging level excludes sensitive data from log output
