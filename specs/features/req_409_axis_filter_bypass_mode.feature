@REQ-409 @product
Feature: Axis Filter Bypass Mode — Disable All Processing for an Axis

  @AC-409.1
  Scenario: Bypass mode passes raw HID value directly to output (normalized only)
    Given an axis with bypass mode enabled
    When a raw HID value is received
    Then the output SHALL be the normalized raw value with no other processing applied

  @AC-409.2
  Scenario: Bypass mode can be toggled per-axis without service restart
    Given a running service with an active axis
    When bypass mode is toggled on or off
    Then the change SHALL take effect immediately without restarting the service

  @AC-409.3
  Scenario: Bypass mode is indicated in flightctl axis status output
    Given an axis with bypass mode enabled
    When the user runs `flightctl axis status`
    Then the output SHALL clearly indicate that bypass mode is active for that axis

  @AC-409.4
  Scenario: Bypass mode is useful for debugging and calibration
    Given an axis in bypass mode
    When raw input values are observed in diagnostics
    Then the unfiltered values SHALL be visible, supporting debugging and calibration workflows

  @AC-409.5
  Scenario: Entering bypass mode does not cause output discontinuity larger than 0.1
    Given an axis currently producing a filtered output
    When bypass mode is enabled
    Then the output change at the transition SHALL not exceed 0.1

  @AC-409.6
  Scenario: Bypass state persists across profile hot-reloads until explicitly cleared
    Given an axis with bypass mode enabled
    When the profile is hot-reloaded
    Then bypass mode SHALL remain active until explicitly disabled
