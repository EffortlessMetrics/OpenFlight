Feature: Profile Conflict Resolution
  As a flight simulation enthusiast
  I want profile merge to have configurable conflict resolution
  So that I have predictable control over which axis settings take precedence

  Background:
    Given the OpenFlight service is running
    And multiple profile layers define conflicting axis settings

  Scenario: Conflict resolution strategy is configurable per axis
    Given axis "pitch" has conflict_resolution set to "first-wins" in the profile
    When conflicting pitch settings are merged from two profile layers
    Then the value from the first layer is used for pitch

  Scenario: First-wins and last-wins strategies are supported
    Given axis "roll" has conflict_resolution set to "last-wins"
    When conflicting roll settings are merged from two profile layers
    Then the value from the last layer is used for roll

  Scenario: Conflict is logged with both conflicting values
    When a merge conflict is detected for an axis
    Then a log entry is emitted containing the axis name and both conflicting values

  Scenario: Resolved profile is validated before activation
    Given a merged profile is ready for activation
    When the profile is validated
    Then any invalid axis configuration causes activation to be rejected with an error
