Feature: Hat Switch Configuration
  As a flight simulation enthusiast
  I want hat switch configuration
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Hat switch behaviors are configurable between 4-way and 8-way modes
    Given the system is configured for hat switch configuration
    When the feature is exercised
    Then hat switch behaviors are configurable between 4-way and 8-way modes

  Scenario: Hat positions can be mapped to arbitrary button or axis outputs
    Given the system is configured for hat switch configuration
    When the feature is exercised
    Then hat positions can be mapped to arbitrary button or axis outputs

  Scenario: Hat switch dead zone is configurable for diagonal rejection
    Given the system is configured for hat switch configuration
    When the feature is exercised
    Then hat switch dead zone is configurable for diagonal rejection

  Scenario: Multiple hat switches on a single device are independently configurable
    Given the system is configured for hat switch configuration
    When the feature is exercised
    Then multiple hat switches on a single device are independently configurable