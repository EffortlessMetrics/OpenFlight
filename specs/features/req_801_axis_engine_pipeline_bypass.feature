Feature: Axis Engine Pipeline Bypass
  As a flight simulation enthusiast
  I want axis engine pipeline bypass
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Enable or disable pipeline stages per-axis
    Given the system is configured for axis engine pipeline bypass
    When the feature is exercised
    Then individual pipeline stages can be enabled or disabled per-axis via configuration

  Scenario: Bypassed stages pass input unmodified
    Given the system is configured for axis engine pipeline bypass
    When the feature is exercised
    Then bypassed stages pass input through unmodified to the next stage

  Scenario: Bypass changes apply within one tick
    Given the system is configured for axis engine pipeline bypass
    When the feature is exercised
    Then pipeline bypass changes take effect within one processing tick

  Scenario: Bypass state persists across restarts
    Given the system is configured for axis engine pipeline bypass
    When the feature is exercised
    Then bypass state is persisted across service restarts
