@REQ-15
Feature: Thrustmaster T.Flight HOTAS 4 parsing and yaw semantics

  @AC-15.2
  Scenario: Parse merged-mode report
    Given a HOTAS 4 input handler
    And a merged-mode report fixture "merged_centered"
    When I parse the report
    Then rocker SHALL be absent
    And hat SHALL equal 0
    And button mask SHALL equal 0

  @AC-15.3
  Scenario: Parse separate-mode report
    Given a HOTAS 4 input handler
    And a separate-mode report fixture "separate_centered"
    When I parse the report
    Then rocker SHALL be present
    And hat SHALL equal 0
    And button mask SHALL equal 0

  @AC-15.4
  Scenario: Axis mode switches mid-session without restart
    Given a HOTAS 4 input handler
    When I parse fixture "merged_centered"
    Then axis mode SHALL equal "Merged"
    When I parse fixture "separate_centered"
    Then axis mode SHALL equal "Separate"
    When I parse fixture "merged_centered"
    Then axis mode SHALL equal "Merged"

  @AC-15.5
  Scenario: Resolve yaw source under Auto policy
    Given a HOTAS 4 input handler with yaw policy "Auto"
    And a separate-mode report fixture "separate_aux_dominant"
    When I parse the report
    Then resolved yaw source SHALL equal "Aux"
