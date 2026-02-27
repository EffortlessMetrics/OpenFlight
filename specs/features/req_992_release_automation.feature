Feature: Release Automation
  As a flight simulation enthusiast
  I want release automation
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Automated release pipeline builds, tests, and publishes release artifacts
    Given the system is configured for release automation
    When the feature is exercised
    Then automated release pipeline builds, tests, and publishes release artifacts

  Scenario: Release pipeline enforces all quality gates before artifact publication
    Given the system is configured for release automation
    When the feature is exercised
    Then release pipeline enforces all quality gates before artifact publication

  Scenario: Semantic versioning is automatically determined from commit history
    Given the system is configured for release automation
    When the feature is exercised
    Then semantic versioning is automatically determined from commit history

  Scenario: Release artifacts are published to configured distribution channels
    Given the system is configured for release automation
    When the feature is exercised
    Then release artifacts are published to configured distribution channels