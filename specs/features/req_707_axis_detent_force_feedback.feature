@REQ-707
Feature: Axis Detent Force Feedback
  @AC-707.1
  Scenario: FFB-capable devices produce tactile feedback at detent positions
    Given the system is configured for REQ-707
    When the feature condition is met
    Then ffb-capable devices produce tactile feedback at detent positions

  @AC-707.2
  Scenario: Detent force magnitude is configurable per detent
    Given the system is configured for REQ-707
    When the feature condition is met
    Then detent force magnitude is configurable per detent

  @AC-707.3
  Scenario: Force feedback respects FFB safety envelope
    Given the system is configured for REQ-707
    When the feature condition is met
    Then force feedback respects ffb safety envelope

  @AC-707.4
  Scenario: Non-FFB devices use output snapping as detent simulation
    Given the system is configured for REQ-707
    When the feature condition is met
    Then non-ffb devices use output snapping as detent simulation
