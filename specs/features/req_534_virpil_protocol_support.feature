@REQ-534 @product
Feature: VIRPIL Device Protocol Support — VIRPIL VPC HID Protocol Integration  @AC-534.1
  Scenario: VIRPIL VPC devices are identified by VID 0x3344
    Given a HID device enumeration that includes a device with VID 0x3344
    When the HID subsystem classifies connected devices
    Then the device SHALL be tagged with the VIRPIL vendor class  @AC-534.2
  Scenario: VIRPIL 14-bit axis resolution is correctly normalised
    Given a VIRPIL VPC device reporting a 14-bit axis value of 8192
    When the axis normalisation stage processes the raw value
    Then the normalised output SHALL be 0.5 within floating point tolerance  @AC-534.3
  Scenario: VIRPIL HID report configuration is documented
    Given the VIRPIL protocol documentation in the compatibility manifest
    When the manifest is validated
    Then it SHALL contain HID report format descriptions for all VIRPIL configuration commands  @AC-534.4
  Scenario: VIRPIL compatibility manifest includes all known PIDs
    Given the VIRPIL compatibility manifest file
    When the manifest is parsed
    Then it SHALL list at least the known VPC WarBRD, VPC MongoosT-50, and VPC Alpha stick PIDs
