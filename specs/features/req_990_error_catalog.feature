Feature: Error Catalog
  As a flight simulation enthusiast
  I want error catalog
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Comprehensive error code reference documents all system error conditions
    Given the system is configured for error catalog
    When the feature is exercised
    Then comprehensive error code reference documents all system error conditions

  Scenario: Each error code includes description, cause, and recommended resolution
    Given the system is configured for error catalog
    When the feature is exercised
    Then each error code includes description, cause, and recommended resolution

  Scenario: Error catalog is generated from source code annotations automatically
    Given the system is configured for error catalog
    When the feature is exercised
    Then error catalog is generated from source code annotations automatically

  Scenario: Error codes follow hierarchical naming convention by subsystem
    Given the system is configured for error catalog
    When the feature is exercised
    Then error codes follow hierarchical naming convention by subsystem