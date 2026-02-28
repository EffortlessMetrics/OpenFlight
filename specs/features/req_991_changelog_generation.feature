Feature: Changelog Generation
  As a flight simulation enthusiast
  I want changelog generation
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Automated changelog is generated from conventional commit messages
    Given the system is configured for changelog generation
    When the feature is exercised
    Then automated changelog is generated from conventional commit messages

  Scenario: Changelog entries are categorized by type including features, fixes, and breaking changes
    Given the system is configured for changelog generation
    When the feature is exercised
    Then changelog entries are categorized by type including features, fixes, and breaking changes

  Scenario: Generated changelog includes links to relevant pull requests and issues
    Given the system is configured for changelog generation
    When the feature is exercised
    Then generated changelog includes links to relevant pull requests and issues

  Scenario: Changelog generation is integrated into the release pipeline
    Given the system is configured for changelog generation
    When the feature is exercised
    Then changelog generation is integrated into the release pipeline