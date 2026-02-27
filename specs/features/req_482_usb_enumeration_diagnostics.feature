@REQ-482 @product
Feature: USB Enumeration Diagnostics — HID Device Enumeration Logging  @AC-482.1
  Scenario: On startup service logs all connected HID devices with VID/PID
    Given multiple HID devices are connected to the system
    When the service starts
    Then the service log SHALL contain an entry for each connected HID device including its VID and PID  @AC-482.2
  Scenario: Known devices are identified by name from compat manifests
    Given a connected device whose VID/PID is present in a compat manifest
    When the service enumerates HID devices at startup
    Then the log entry for that device SHALL include the human-readable name from the manifest  @AC-482.3
  Scenario: Unknown devices are logged with VID/PID and capabilities
    Given a connected HID device not present in any compat manifest
    When the service enumerates HID devices at startup
    Then the log entry SHALL include the raw VID/PID and a summary of the device's reported capabilities  @AC-482.4
  Scenario: Enumeration results are included in diagnostic bundle
    Given the service has completed startup enumeration
    When a diagnostic bundle is collected via `flightctl diag bundle`
    Then the bundle SHALL include the full HID enumeration log from the most recent startup
