Feature: Axis Input Lag Compensation
  As a flight simulation enthusiast
  I want the axis engine to support input lag compensation
  So that control inputs feel immediate despite system latency

  Background:
    Given the OpenFlight service is running
    And the axis engine is active at 250 Hz

  Scenario: Lag compensation predicts future axis position using velocity
    Given lag compensation is enabled for the pitch axis
    And the pitch axis has a measured velocity of 0.1 per tick
    When the axis engine processes a tick
    Then the compensated pitch output is offset by the predicted position change

  Scenario: Prediction horizon is configurable in milliseconds
    Given the lag compensation prediction horizon is set to 20 milliseconds
    When the axis engine computes the compensated output
    Then the prediction uses a 20 millisecond look-ahead based on current velocity

  Scenario: Lag compensation is disabled when axis is at rest
    Given the pitch axis has been stationary for 5 consecutive ticks
    When the axis engine processes the next tick
    Then lag compensation is not applied to the pitch axis output

  Scenario: Compensated output remains within valid range
    Given a high-velocity axis movement near the maximum position
    When lag compensation is applied
    Then the compensated output is clamped to the range -1.0 to 1.0
