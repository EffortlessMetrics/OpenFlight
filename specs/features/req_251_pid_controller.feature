@REQ-251 @product
Feature: PID controller available as axis processing stage for stable outputs  @AC-251.1
  Scenario: PID controller configurable with P I and D gains per axis
    Given an axis profile with pid.p, pid.i, and pid.d values specified
    When the axis processing pipeline is initialised
    Then the PID stage SHALL load the configured gains and use them during processing  @AC-251.2
  Scenario: Integral windup limited to configurable maximum
    Given a PID controller with a configured integral windup limit
    When the error signal persists and the integral accumulator would exceed the limit
    Then the accumulator SHALL be clamped to the configured maximum  @AC-251.3
  Scenario: Derivative term smoothed with low-pass filter to reduce noise
    Given a PID controller processing a noisy axis signal
    When the derivative term is calculated each tick
    Then the derivative SHALL be passed through a configurable low-pass filter before use  @AC-251.4
  Scenario: PID setpoint configurable as profile parameter or runtime value
    Given a profile with a static pid.setpoint field
    When the profile is loaded
    Then the PID stage SHALL use the profile setpoint unless overridden by a runtime command  @AC-251.5
  Scenario: PID state resets on axis disconnect
    Given a PID controller with a non-zero integral accumulator
    When the axis device disconnects
    Then the integral accumulator SHALL be reset to zero  @AC-251.6
  Scenario: PID output bounded to minus one point zero to one point zero
    Given a PID controller whose computed output exceeds the normalised range
    When the output is produced for the next pipeline stage
    Then the output SHALL be clamped to the range [-1.0, 1.0]
