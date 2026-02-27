@REQ-515 @product
Feature: HID Descriptor Caching

  @AC-515.1 @AC-515.2
  Scenario: Descriptor is cached and validated against device serial number
    Given a HID device connects for the first time
    When the device descriptor is read and cached to disk
    Then subsequent enumerations SHALL load from cache if the serial number matches

  @AC-515.3
  Scenario: Cache is invalidated when firmware version changes
    Given a cached descriptor for a device exists on disk
    When the device reconnects with a different firmware version
    Then the cache SHALL be invalidated and the descriptor re-read from the device

  @AC-515.4
  Scenario: Cache miss falls back to live descriptor read
    Given no cached descriptor exists for a newly connected device
    When the HID subsystem enumerates the device
    Then the descriptor SHALL be read live from the device and then cached
