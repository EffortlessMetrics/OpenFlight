Feature: Axis Position Dead Band Testing
  As a flight simulation developer
  I want dedicated dead band boundary tests in the axis engine
  So that I can verify that deadband transitions are correct and consistent

  Background:
    Given the OpenFlight service is running

  Scenario: Property tests verify deadband transition is monotonic
    When property-based tests run for deadband transition
    Then all transition outputs are monotonically non-decreasing

  Scenario: Edge values at deadband boundary produce consistent output
    Given the axis engine dead band is configured
    When an axis value exactly at the deadband boundary is processed
    Then the output is consistent on repeated evaluation

  Scenario: Tests cover both positive and negative dead band sides
    When the axis dead band test suite is executed
    Then test cases exist for both positive and negative boundary sides

  Scenario: Dead band tests run on every PR
    When a pull request is opened against the repository
    Then the dead band test suite is included in the CI test run
