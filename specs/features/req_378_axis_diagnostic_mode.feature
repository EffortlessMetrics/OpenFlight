@REQ-378 @product
Feature: Axis Diagnostic Mode — Emit Verbose Per-Tick Diagnostic Telemetry

  @AC-378.1
  Scenario: Diagnostic mode is enabled per axis via flightctl
    Given a running service with an active axis
    When the user runs `flightctl axis diag --enable <axis>`
    Then diagnostic mode SHALL be activated for that axis

  @AC-378.2
  Scenario: Per-tick data is published to the diagnostics bus channel in diagnostic mode
    Given an axis with diagnostic mode enabled
    When each RT tick is processed
    Then per-tick diagnostic data SHALL be published to the diagnostics bus channel

  @AC-378.3
  Scenario: Diagnostic data includes all pipeline stage values
    Given an axis with diagnostic mode enabled
    When a tick is processed
    Then the diagnostic payload SHALL include tick index, raw, post-calibration, post-deadzone, post-curve, and final values

  @AC-378.4
  Scenario: Diagnostic mode does not impact RT performance when disabled
    Given an axis with diagnostic mode disabled
    When the axis is measured under RT conditions
    Then the per-tick processing time SHALL be within the normal RT budget

  @AC-378.5
  Scenario: Diagnostic subscribers can connect and disconnect without interrupting the RT loop
    Given an axis with diagnostic mode enabled and a subscriber connected
    When the subscriber disconnects
    Then the RT loop SHALL continue processing without interruption

  @AC-378.6
  Scenario: Diagnostic mode automatically disables after 60 seconds
    Given an axis with diagnostic mode enabled
    When 60 seconds elapse
    Then diagnostic mode SHALL automatically disable to prevent log flooding
