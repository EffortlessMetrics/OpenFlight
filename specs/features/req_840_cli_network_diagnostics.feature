Feature: CLI Network Diagnostics
  As a flight simulation enthusiast
  I want cli network diagnostics
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Diagnose connectivity to sim adapters
    Given the system is configured for cli network diagnostics
    When the feature is exercised
    Then cLI diagnoses network connectivity to configured sim adapters

  Scenario: Report latency and packet loss
    Given the system is configured for cli network diagnostics
    When the feature is exercised
    Then diagnostics report latency and packet loss for each adapter endpoint

  Scenario: Display structured summary table
    Given the system is configured for cli network diagnostics
    When the feature is exercised
    Then results are displayed in a structured summary table

  Scenario: Complete within configurable timeout
    Given the system is configured for cli network diagnostics
    When the feature is exercised
    Then diagnostic command completes within a configurable timeout
