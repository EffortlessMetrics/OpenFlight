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

  @AC-16.3
  Scenario: Out-of-range HAT value is clamped to center
    Given a HOTAS 4 input handler
    When I parse fixture "hat_out_of_range"
    Then HAT SHALL equal 0

  @AC-16.3
  Scenario: HAT value 8 is preserved as a valid direction
    Given a HOTAS 4 input handler
    When I parse fixture "hat_max_valid"
    Then HAT SHALL equal 8

  @AC-16.4
  Scenario: Throttle inversion maps minimum throttle to maximum
    Given a HOTAS 4 input handler with throttle inversion enabled
    When I parse fixture "throttle_min"
    Then throttle SHALL be approximately 1.0
