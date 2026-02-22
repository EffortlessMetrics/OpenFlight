@REQ-16
Feature: T.Flight HOTAS 4 runtime auto-mode and report-ID handling

  @AC-16.1
  Scenario: Runtime auto-detects merged mode on first report
    Given a HOTAS 4 input handler
    When I parse fixture "merged_centered"
    Then axis mode SHALL equal "Merged"

  @AC-16.1
  Scenario: Runtime auto-detects separate mode on first report
    Given a HOTAS 4 input handler
    When I parse fixture "separate_centered"
    Then axis mode SHALL equal "Separate"

  @AC-16.2
  Scenario: Report ID prefix is stripped before parsing merged report
    Given a HOTAS 4 input handler with report ID enabled
    When I parse fixture "report_id_merged_centered"
    Then axis mode SHALL equal "Merged"
    And rocker SHALL be absent

  @AC-16.2
  Scenario: Report ID prefix is stripped before parsing separate report
    Given a HOTAS 4 input handler with report ID enabled
    When I parse fixture "report_id_separate_centered"
    Then axis mode SHALL equal "Separate"
    And rocker SHALL be present
