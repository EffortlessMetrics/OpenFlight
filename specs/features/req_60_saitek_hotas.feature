@REQ-60
Feature: Saitek HOTAS HID parsing

  @AC-60.1
  Scenario: X52 and X56 reports parse axes correctly
    Given an HotasInputHandler for the X52 device type
    When a 14-byte report with throttle byte set to 255 is parsed
    Then the throttle axis SHALL be greater than 0.9
    And an HotasInputHandler for X56Throttle SHALL parse dual independent throttle axes
    And device_type() SHALL return the type supplied at construction

  @AC-60.2
  Scenario: Axis normalisation produces values in −1.0..1.0
    Given the normalize_axis_8bit and normalize_axis_16bit helper functions
    When called with minimum, mid-scale, and maximum raw values
    Then 8-bit minimum (0) SHALL produce approximately −1.0
    And 8-bit mid-scale (127) SHALL produce approximately 0.0
    And 8-bit maximum (255) SHALL produce approximately 1.0
    And 16-bit minimum (0) SHALL produce approximately −1.0
    And 16-bit mid-scale (32767) SHALL produce approximately 0.0
    And 16-bit maximum (65535) SHALL produce approximately 1.0

  @AC-60.3
  Scenario: Health monitor tracks failures and ghost input rates
    Given a HotasHealthMonitor for any Saitek device
    When successes are recorded the device SHALL be marked healthy
    And when three consecutive failures are recorded the monitor SHALL track them
    Then ghost_rate() SHALL reflect the proportion of ghost inputs seen
    And health status SHALL indicate ghost issues when the ghost rate is non-zero

  @AC-60.4
  Scenario: Short reports return a zero default state
    Given an HotasInputHandler for the X52 device type
    When a report of only 10 bytes is parsed (minimum is 14)
    Then all axis fields in the returned state SHALL be 0.0
    And no panic SHALL occur
    And an X55 stick handler SHALL similarly return zero state for a 5-byte report

  @AC-60.5
  Scenario: Output I/O policy requires opt-in environment variable
    Given the Saitek output I/O policy is evaluated without the opt-in env variable set
    Then allow_device_io SHALL return false
    When the opt-in environment variable is set
    Then allow_device_io SHALL return true
    And after the cache is reset allow_device_io SHALL re-evaluate without relying on a stale cached result
