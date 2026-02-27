Feature: Documentation Site
  As a flight simulation enthusiast
  I want documentation site
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Generated documentation site includes API reference and user guides
    Given the system is configured for documentation site
    When the feature is exercised
    Then generated documentation site includes API reference and user guides

  Scenario: Documentation is built from source and published on every release
    Given the system is configured for documentation site
    When the feature is exercised
    Then documentation is built from source and published on every release

  Scenario: Site search enables finding content across all documentation sections
    Given the system is configured for documentation site
    When the feature is exercised
    Then site search enables finding content across all documentation sections

  Scenario: Documentation site supports versioned content for multiple releases
    Given the system is configured for documentation site
    When the feature is exercised
    Then documentation site supports versioned content for multiple releases