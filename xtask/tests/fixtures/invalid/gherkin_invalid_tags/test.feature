@REQ-999
Feature: Feature with invalid tags

  This feature has tags that don't exist in the spec ledger.

  @AC-999.1
  Scenario: Scenario with invalid AC tag
    Given a precondition
    When an action occurs
    Then a result is expected

  @REQ-888 @AC-888.1
  Scenario: Scenario with multiple invalid tags
    Given another precondition
    When another action occurs
    Then another result is expected
