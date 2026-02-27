@REQ-277 @infra
Feature: Fuzz coverage reporting enforces 70% threshold per target and persists corpus between nightly runs  @AC-277.1
  Scenario: CI nightly reports fuzz coverage percentage per target
    Given the nightly CI pipeline has completed fuzz runs for all registered targets
    When the coverage report step executes
    Then a coverage percentage SHALL be published for each fuzz target  @AC-277.2
  Scenario: Coverage below 70 percent on a fuzz target fails the nightly gate
    Given a fuzz target whose line coverage is below 70%
    When the nightly coverage gate is evaluated
    Then the nightly pipeline SHALL fail and report the under-covered target  @AC-277.3
  Scenario: Fuzz corpus is persisted between nightly runs
    Given a nightly fuzz run has completed and produced corpus entries
    When the next nightly run begins
    Then the prior corpus SHALL be available as seed inputs for the fuzz targets  @AC-277.4
  Scenario: New crash findings are captured as regression tests
    Given a fuzz run discovers a new crash input
    When the nightly pipeline processes the finding
    Then the crashing input SHALL be committed as a regression test in the repository  @AC-277.5
  Scenario: Fuzz summary is included in CI run artifacts
    Given a nightly fuzz run has completed
    When CI artifacts are published
    Then a fuzz summary file SHALL be included listing coverage, corpus size, and any new findings  @AC-277.6
  Scenario: Each parser crate has at least one fuzz target
    Given the list of crates that implement parsing logic
    When the fuzz target registry is inspected
    Then every parser crate SHALL have at least one registered fuzz target
