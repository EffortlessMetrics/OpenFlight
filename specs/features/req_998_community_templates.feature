Feature: Community Templates
  As a flight simulation enthusiast
  I want community templates
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Issue templates guide reporters through structured bug and feature submissions
    Given the system is configured for community templates
    When the feature is exercised
    Then issue templates guide reporters through structured bug and feature submissions

  Scenario: Pull request template includes checklist for quality gate compliance
    Given the system is configured for community templates
    When the feature is exercised
    Then pull request template includes checklist for quality gate compliance

  Scenario: Discussion templates support categorized community conversations
    Given the system is configured for community templates
    When the feature is exercised
    Then discussion templates support categorized community conversations

  Scenario: Templates are validated for completeness and updated with project evolution
    Given the system is configured for community templates
    When the feature is exercised
    Then templates are validated for completeness and updated with project evolution