Feature: Profile Variable Substitution
  As a flight simulation enthusiast
  I want profile variable substitution
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Dollar-brace variable syntax
    Given the system is configured for profile variable substitution
    When the feature is exercised
    Then profile yaml supports dollar-brace variable references

  Scenario: Resolve from environment
    Given the system is configured for profile variable substitution
    When the feature is exercised
    Then variables resolve from environment at profile load time

  Scenario: Error on unresolved variables
    Given the system is configured for profile variable substitution
    When the feature is exercised
    Then unresolved variables cause a validation error

  Scenario: Substitution in all string fields
    Given the system is configured for profile variable substitution
    When the feature is exercised
    Then variable substitution works in all string-valued fields
