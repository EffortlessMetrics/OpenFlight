@REQ-444 @product
Feature: USB Vendor Product ID Database — Map USB VID/PID Pairs to Known Device Info

  @AC-444.1
  Scenario: Compat manifest directory provides VID/PID to device info mapping
    Given the compat manifest directory contains device entries
    When the device database is initialised
    Then all VID/PID pairs from the manifests SHALL be present in the lookup table

  @AC-444.2
  Scenario: Lookup by VID/PID returns vendor, product name, and capabilities
    Given a known device with VID 0x044F and PID 0xB10A is in the database
    When a lookup is performed for that VID/PID pair
    Then the result SHALL include the vendor name, product name, and device capability flags

  @AC-444.3
  Scenario: Unknown VID/PID returns a generic device info with logging
    Given a device with an unrecognised VID/PID pair is connected
    When the database is queried for that pair
    Then a generic DeviceInfo SHALL be returned and an unknown-device warning SHALL be logged

  @AC-444.4
  Scenario: Database is compiled into a static lookup table at build time
    Given the compat manifests are present at build time
    When the crate is compiled
    Then the VID/PID database SHALL be embedded as a static table with no runtime file I/O required
