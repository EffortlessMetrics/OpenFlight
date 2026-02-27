Feature: Service Startup Dependencies
  As a flight simulation enthusiast
  I want service startup to declare and resolve component dependencies
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Dependencies are declared
    Given components have declared their dependencies
    When the service starts
    Then the dependency graph is constructed

  Scenario: Components start in order
    Given a dependency graph is resolved
    When the service starts components
    Then each component starts after its dependencies

  Scenario: Circular dependencies detected
    Given a circular dependency exists
    When the service attempts to start
    Then the cycle is detected and reported

  Scenario: Failed dependency blocks dependents
    Given a component fails to start
    When dependent components attempt to start
    Then they are prevented from starting
