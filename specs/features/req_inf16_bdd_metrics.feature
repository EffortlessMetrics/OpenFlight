@INF-REQ-16
Feature: BDD traceability metrics library

  @AC-16.1
  Scenario: Coverage methods return correct percentages
    Given a BddTraceabilityRow with some ACs having gherkin
    When coverage percent is called
    Then it SHALL return the ratio of covered to total ACs

  @AC-16.1
  Scenario: Coverage percent with zero denominator returns zero
    Given a BddTraceabilityRow with no ACs
    When coverage percent is called
    Then it SHALL return zero without dividing by zero

  @AC-16.2
  Scenario: Scenario header variants are recognized
    Given lines starting with Scenario Background and Scenario Outline
    When is_scenario_header is called
    Then each SHALL be recognized as a header

  @AC-16.2
  Scenario: AC tag filtering removes non-AC tags
    Given a Gherkin scenario with mixed tags including non-AC tags
    When AC tags are extracted
    Then only AC-prefixed tags SHALL be returned

  @AC-16.3
  Scenario: Double-colon test references are parsed to crate names
    Given a test reference in double-colon notation like flight-core::module::test
    When crates are extracted from the reference
    Then flight-core SHALL be in the extracted crate names

  @AC-16.3
  Scenario: Command references with -p flag are parsed
    Given a command reference like cmd:cargo test -p flight-core
    When crates are extracted from the command
    Then flight-core SHALL be in the extracted crate names

  @AC-16.4
  Scenario: Coverage status computes all AC state branches
    Given acceptance criteria in various states
    When coverage status is computed
    Then each combination of test and gherkin coverage SHALL produce the correct AcStatus

  @AC-16.4
  Scenario: Workspace crate filtering preserves unmapped rows
    Given a BDD metrics set with an unmapped row
    When workspace crate filtering is applied
    Then the unmapped row SHALL be preserved regardless of workspace crate membership
