Feature: Service Memory Budgets
  As a flight simulation enthusiast
  I want the service to enforce per-component memory budgets
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Memory budgets are configurable
    Given the service configuration specifies memory budgets
    When the service starts
    Then each component is assigned its configured memory budget

  Scenario: Over-allocation is prevented
    Given a component reaches its memory budget
    When it attempts to allocate more memory
    Then the allocation is denied

  Scenario: Violations are logged
    Given a component exceeds its memory budget
    When the violation is detected
    Then it is logged with the component identity

  Scenario: Usage queryable via IPC
    Given the service is running
    When a client queries memory usage via IPC
    Then current per-component memory usage is returned
