@REQ-400 @product
Feature: 400-Requirement Milestone — System is Comprehensively Specified

  @AC-400.1
  Scenario: Spec ledger contains 400 or more requirements
    Given the spec_ledger.yaml file
    When the total number of defined requirements is counted
    Then the count SHALL be 400 or more

  @AC-400.2
  Scenario: Each requirement has exactly 6 acceptance criteria
    Given every requirement in the spec ledger
    When the acceptance criteria count is checked
    Then each requirement SHALL have exactly 6 acceptance criteria

  @AC-400.3
  Scenario: Each requirement has a corresponding BDD feature file
    Given every requirement in the spec ledger
    When the feature file referenced in test_ref is looked up
    Then a corresponding feature file SHALL exist on disk

  @AC-400.4
  Scenario: All feature files are valid Gherkin syntax
    Given all feature files in specs/features/
    When each file is parsed by a Gherkin parser
    Then all files SHALL parse without syntax errors

  @AC-400.5
  Scenario: Spec ledger passes automated schema validation
    Given the spec_ledger.yaml file
    When `cargo xtask validate` is executed
    Then the spec ledger SHALL pass all schema validation checks

  @AC-400.6
  Scenario: Requirements coverage matrix is generated and available in docs/
    Given the spec ledger and associated feature files
    When the coverage matrix generation step is run
    Then a coverage matrix document SHALL be present in docs/
