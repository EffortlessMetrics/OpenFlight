@REQ-1
Feature: Test Feature for Requirement 1

  This is a test feature for validating Gherkin parsing.

  @AC-1.1
  Scenario: First acceptance criteria test
    Given a system in initial state
    When an action is performed
    Then the expected result occurs

  @AC-1.2 @smoke
  Scenario: Second acceptance criteria test
    Given another precondition
    When another action occurs
    Then another result is expected
