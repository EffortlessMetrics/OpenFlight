@REQ-323 @product
Feature: Device Configuration Profiles  @AC-323.1
  Scenario: Each device can have a device-level config independent of aircraft profile
    Given a HID device is connected and a device config exists for it
    When the active aircraft profile is changed
    Then the device-level configuration SHALL remain unchanged  @AC-323.2
  Scenario: Device config specifies polling rate report format and calibration
    Given a device config file for a connected device
    When the service loads the config
    Then the device SHALL operate using the polling rate, report format, and calibration values from its device config  @AC-323.3
  Scenario: Device configs survive aircraft profile switches
    Given a device config with a custom polling rate is active
    When the user switches to a different aircraft profile
    Then the device SHALL continue to use the polling rate from its device config  @AC-323.4
  Scenario: Device config is included in flightctl export --devices output
    Given one or more device configs are present
    When the user runs flightctl export --devices
    Then the exported output SHALL include the device configuration for each configured device  @AC-323.5
  Scenario: Device config supports per-axis polarity override
    Given a device config that specifies inverted polarity for axis 0
    When input is read from axis 0
    Then the reported axis value SHALL have its polarity inverted relative to the raw hardware value  @AC-323.6
  Scenario: Device config changes take effect without service restart
    Given the service is running with an existing device config
    When the device config file is updated
    Then the service SHALL apply the new config values to the device without requiring a restart
