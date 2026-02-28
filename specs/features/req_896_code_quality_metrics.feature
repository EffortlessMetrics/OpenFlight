Feature: Code Quality Metrics
  As a flight simulation enthusiast
  I want code quality metrics
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Cyclomatic complexity is tracked and reported per module
    Given the system is configured for code quality metrics
    When the feature is exercised
    Then cyclomatic complexity is tracked and reported per module

  Scenario: Complexity trends are compared against previous release baselines
    Given the system is configured for code quality metrics
    When the feature is exercised
    Then complexity trends are compared against previous release baselines

  Scenario: Modules exceeding the complexity threshold are flagged for review
    Given the system is configured for code quality metrics
    When the feature is exercised
    Then modules exceeding the complexity threshold are flagged for review

  Scenario: Metrics are collected automatically during CI builds
    Given the system is configured for code quality metrics
    When the feature is exercised
    Then metrics are collected automatically during CI builds
