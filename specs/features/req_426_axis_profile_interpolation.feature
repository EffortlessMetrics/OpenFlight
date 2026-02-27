@REQ-426 @product
Feature: Axis Profile Interpolation — Interpolate Between Two Profiles for Smooth Transitions

  @AC-426.1
  Scenario: Profile transition blends axis configs over a configurable duration
    Given two profiles with different axis configurations
    When a profile transition is triggered with a specified duration
    Then the axis config SHALL blend smoothly over that duration

  @AC-426.2
  Scenario: Transition duration is configurable per axis with 0 meaning instant
    Given a profile with per-axis transition_duration fields
    When the profile is loaded
    Then axes with duration 0 SHALL switch instantly while others blend over the specified time

  @AC-426.3
  Scenario: During transition axis deadzone and curve are linearly interpolated
    Given an active profile transition
    When intermediate ticks are processed
    Then the deadzone and curve settings SHALL be the linear interpolation between source and target profiles

  @AC-426.4
  Scenario: Transition completes within the specified duration measured in RT ticks
    Given a transition with a duration of N ticks
    When N ticks have elapsed
    Then the axis config SHALL fully match the target profile with no residual blend

  @AC-426.5
  Scenario: New profile applied during transition becomes the new transition target
    Given an in-progress profile transition
    When a new profile change is applied before the transition completes
    Then the current interpolated state SHALL become the new source and the new profile the target

  @AC-426.6
  Scenario: Transition state is reset on service restart
    Given a service restart occurring during an active profile transition
    When the service restarts
    Then it SHALL load the target profile directly with no residual transition state
