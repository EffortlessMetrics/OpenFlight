Feature: License Compliance
  As a flight simulation enthusiast
  I want license compliance
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: SPDX license tracking covers all direct and transitive dependencies
    Given the system is configured for license compliance
    When the feature is exercised
    Then sPDX license tracking covers all direct and transitive dependencies

  Scenario: License compatibility is validated against project license requirements
    Given the system is configured for license compliance
    When the feature is exercised
    Then license compatibility is validated against project license requirements

  Scenario: License report is generated and included in distribution artifacts
    Given the system is configured for license compliance
    When the feature is exercised
    Then license report is generated and included in distribution artifacts

  Scenario: New dependency additions trigger automatic license review in CI
    Given the system is configured for license compliance
    When the feature is exercised
    Then new dependency additions trigger automatic license review in CI