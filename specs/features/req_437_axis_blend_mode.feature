@REQ-437 @product
Feature: Axis Blend Mode Selection — Per-Axis Blend Mode for Combining Multiple Physical Inputs

  @AC-437.1
  Scenario: Blend modes include max, min, add, average, and first-active
    Given a virtual axis configured with each blend mode in turn
    When two physical inputs contribute values
    Then the output SHALL match the expected result for max, min, add, average, and first-active modes

  @AC-437.2
  Scenario: Blend mode is configurable per axis in profile
    Given a profile with different blend modes set on different axes
    When the profile is loaded
    Then each axis SHALL use its individually specified blend mode

  @AC-437.3
  Scenario: Blend applies when multiple physical inputs map to the same virtual axis
    Given two physical axes mapped to the same virtual axis with blend mode average
    When both physical axes report values
    Then the virtual axis output SHALL be the average of the two input values

  @AC-437.4
  Scenario: Blend mode change takes effect at next RT tick without glitches
    Given a running axis pipeline with blend mode set to max
    When the blend mode is changed to min in the profile
    And the new profile is atomically swapped at a tick boundary
    Then the output SHALL immediately use min blending with no output discontinuity
