Feature: Device Usage Analytics
  As a flight simulation enthusiast
  I want device usage analytics
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Device usage patterns are tracked including active time per axis
    Given the system is configured for device usage analytics
    When the feature is exercised
    Then device usage patterns are tracked including active time per axis

  Scenario: Analytics data is stored locally with configurable retention
    Given the system is configured for device usage analytics
    When the feature is exercised
    Then analytics data is stored locally with configurable retention

  Scenario: Usage reports can be generated per device and per session
    Given the system is configured for device usage analytics
    When the feature is exercised
    Then usage reports can be generated per device and per session

  Scenario: Analytics collection can be disabled entirely by the user
    Given the system is configured for device usage analytics
    When the feature is exercised
    Then analytics collection can be disabled entirely by the user
