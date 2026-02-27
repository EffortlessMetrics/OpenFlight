Feature: 600-Requirement Milestone
  As a flight simulation enthusiast
  I want the OpenFlight specification to reach 600 requirements
  So that the specification comprehensively covers all system behaviours

  Background:
    Given the spec_ledger.yaml file is accessible

  Scenario: spec_ledger.yaml contains 600 or more requirements
    When the ledger is parsed
    Then it contains 600 or more requirement entries

  Scenario: All requirements have unique IDs
    When the ledger is parsed
    Then every requirement ID appears exactly once

  Scenario: All requirements have at least one acceptance criterion
    When the ledger is parsed
    Then every requirement entry has at least one acceptance criterion defined

  Scenario: All feature files exist for all ledger entries
    When the ledger is parsed
    Then every referenced feature file path resolves to an existing file on disk
