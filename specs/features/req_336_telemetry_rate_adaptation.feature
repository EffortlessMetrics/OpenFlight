@REQ-336 @product
Feature: Telemetry Rate Adaptation  @AC-336.1
  Scenario: Service detects telemetry rate from simulator
    Given a simulator sending telemetry at a measured rate between 10Hz and 200Hz
    When the adapter connects
    Then the service SHALL detect and record the incoming telemetry rate  @AC-336.2
  Scenario: Internal processing resamples to 250Hz
    Given a simulator sending telemetry at 60Hz
    When the adapter receives frames
    Then the service SHALL upsample the data to produce a 250Hz axis processing stream  @AC-336.3
  Scenario: Linear interpolation preserves axis accuracy during upsampling
    Given two consecutive telemetry frames with different axis values
    When upsampling inserts intermediate samples
    Then each intermediate sample SHALL be a linearly interpolated value between the two frames  @AC-336.4
  Scenario: Rate mismatch is logged once on connect
    Given a simulator whose telemetry rate differs from 250Hz
    When the connection is established
    Then the service SHALL log the detected rate and resampling ratio exactly once  @AC-336.5
  Scenario: Adaptation is per-simulator with independent rates
    Given MSFS connected at 60Hz and X-Plane connected at 100Hz simultaneously
    When both adapters process frames
    Then each adapter SHALL independently resample to 250Hz without interfering with each other  @AC-336.6
  Scenario: Zero-rate input clears axes to idle state
    Given a simulator connection that drops to zero telemetry rate (disconnected)
    When no frames are received
    Then the service SHALL clear all axis values to their idle state
