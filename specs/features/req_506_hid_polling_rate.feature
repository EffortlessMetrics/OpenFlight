@REQ-506 @product
Feature: HID Polling Rate Control — Configurable Per-Device Polling Frequency  @AC-506.1
  Scenario: HID polling rate is configurable from 60 Hz to 1000 Hz
    Given a HID device entry in the service configuration
    When the polling rate is set to 500 Hz and the service starts
    Then the device SHALL be polled at 500 Hz  @AC-506.2
  Scenario: Polling rate is applied per device where OS permits
    Given two HID devices configured with different polling rates
    When the service starts and enumerates devices
    Then each device SHALL be polled at its individually configured rate where the OS allows  @AC-506.3
  Scenario: Actual polling rate is measured and reported in metrics
    Given a HID device is being polled by the service
    When the HID metrics endpoint is queried
    Then the response SHALL include the measured actual polling rate for the device  @AC-506.4
  Scenario: Polling rate mismatch triggers a warning
    Given a device configured for 500 Hz polling
    When the measured rate deviates from 500 Hz by more than 10%
    Then the service SHALL log a warning indicating the polling rate mismatch
