@REQ-548 @product
Feature: Adaptive Rate Limiting — Axis engine should adapt rate limiting based on maneuver detection

  @AC-548.1
  Scenario: Maneuver detection identifies high-energy input sequences
    Given an axis stream with a rapid high-amplitude input sequence
    When the maneuver detector processes the sequence
    Then it SHALL classify the sequence as a detected maneuver

  @AC-548.2
  Scenario: Rate limit is relaxed during detected maneuvers
    Given maneuver detection is active and a maneuver is in progress
    When the axis engine applies the rate limiter
    Then the effective rate limit SHALL be relaxed to the configured maneuver-mode value

  @AC-548.3
  Scenario: Rate limit returns to configured value after maneuver ends
    Given a maneuver has been detected and subsequently ended
    When the maneuver exit condition is satisfied
    Then the rate limit SHALL revert to the configured baseline value

  @AC-548.4
  Scenario: Maneuver detection threshold is configurable
    Given the maneuver detection threshold is set in the profile
    When the axis engine initialises
    Then it SHALL use the profile-specified threshold for maneuver classification
