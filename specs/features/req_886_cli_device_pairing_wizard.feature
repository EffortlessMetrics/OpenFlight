Feature: CLI Device Pairing Wizard
  As a flight simulation enthusiast
  I want cli device pairing wizard
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Guided wizard walks user through device setup step by step
    Given the system is configured for cli device pairing wizard
    When the feature is exercised
    Then guided wizard walks user through device setup step by step

  Scenario: Wizard detects connected devices and suggests default configurations
    Given the system is configured for cli device pairing wizard
    When the feature is exercised
    Then wizard detects connected devices and suggests default configurations

  Scenario: Each step validates input before proceeding to the next
    Given the system is configured for cli device pairing wizard
    When the feature is exercised
    Then each step validates input before proceeding to the next

  Scenario: Wizard can be cancelled at any step without partial side effects
    Given the system is configured for cli device pairing wizard
    When the feature is exercised
    Then wizard can be cancelled at any step without partial side effects
