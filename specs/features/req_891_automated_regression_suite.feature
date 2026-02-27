Feature: Automated Regression Suite
  As a flight simulation enthusiast
  I want automated regression suite
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Full regression test suite runs with a single command invocation
    Given the system is configured for automated regression suite
    When the feature is exercised
    Then full regression test suite runs with a single command invocation

  Scenario: Suite covers unit, integration, and BDD feature tests
    Given the system is configured for automated regression suite
    When the feature is exercised
    Then suite covers unit, integration, and BDD feature tests

  Scenario: Test results are reported in JUnit XML format for CI integration
    Given the system is configured for automated regression suite
    When the feature is exercised
    Then test results are reported in JUnit XML format for CI integration

  Scenario: Flaky test detection flags tests with inconsistent pass rates
    Given the system is configured for automated regression suite
    When the feature is exercised
    Then flaky test detection flags tests with inconsistent pass rates
