Feature: Axis Group Phase Coherence
  As a flight simulation enthusiast
  I want axis groups to maintain phase coherence for coordinated inputs
  So that grouped axes move in sync and coherence violations are surfaced

  Background:
    Given the OpenFlight service is running
    And an axis group "RUDDER_PEDALS" contains axes "RUDDER_LEFT" and "RUDDER_RIGHT"

  Scenario: Group phase tracking updates all member axes within the same tick
    When the axis engine processes a tick
    Then both "RUDDER_LEFT" and "RUDDER_RIGHT" phase timestamps are updated in the same tick

  Scenario: Phase difference between group members is monitored
    Given "RUDDER_LEFT" was last updated 2 ticks ago
    And "RUDDER_RIGHT" was updated in the current tick
    When the phase coherence monitor evaluates the group
    Then a phase difference of 2 ticks is recorded for "RUDDER_PEDALS"

  Scenario: Phase coherence violation triggers a diagnostic event
    Given the coherence window for "RUDDER_PEDALS" is 1 tick
    When the phase difference between group members exceeds 1 tick
    Then a "GroupPhaseCoherenceViolation" diagnostic event is emitted for "RUDDER_PEDALS"

  Scenario: Coherence window is configurable per group
    Given the profile sets a coherence window of 3 ticks for "RUDDER_PEDALS"
    When a phase difference of 2 ticks is observed
    Then no coherence violation is raised
    When a phase difference of 4 ticks is observed
    Then a coherence violation diagnostic event is emitted
