Feature: Multi-Sim Simultaneous Operation
  As a flight simulation enthusiast
  I want multi-sim simultaneous operation
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Multiple simulator adapters can be active simultaneously without conflict
    Given the system is configured for multi-sim simultaneous operation
    When the feature is exercised
    Then multiple simulator adapters can be active simultaneously without conflict

  Scenario: Device routing rules determine which sim receives input from each device
    Given the system is configured for multi-sim simultaneous operation
    When the feature is exercised
    Then device routing rules determine which sim receives input from each device

  Scenario: Sim priority ordering resolves conflicts when multiple sims claim same device
    Given the system is configured for multi-sim simultaneous operation
    When the feature is exercised
    Then sim priority ordering resolves conflicts when multiple sims claim same device

  Scenario: Adding or removing a sim adapter does not disrupt other active connections
    Given the system is configured for multi-sim simultaneous operation
    When the feature is exercised
    Then adding or removing a sim adapter does not disrupt other active connections