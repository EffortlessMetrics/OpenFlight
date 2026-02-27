@REQ-420 @product
Feature: HID Report ID Filtering — Handle Devices with Multiple HID Report IDs

  @AC-420.1
  Scenario: Devices with multiple report IDs are parsed per-report-ID
    Given a HID device that uses multiple report IDs
    When a report is received
    Then the parser SHALL select the correct descriptor for that report ID

  @AC-420.2
  Scenario: Report ID 0x00 (no report IDs) is treated as a single-report device
    Given a HID device that does not use report IDs
    When a report is received with ID 0x00
    Then it SHALL be parsed as a single-report device without report-ID stripping

  @AC-420.3
  Scenario: Unknown report IDs are logged and skipped without error
    Given a HID device sending a report with an unrecognized report ID
    When the report is processed
    Then a DEBUG-level log entry SHALL be emitted and the report SHALL be skipped without error

  @AC-420.4
  Scenario: Report ID is stored in parsed axis data for downstream processing
    Given a parsed HID axis value
    When the axis data struct is inspected
    Then it SHALL contain the report_id field from which it was parsed

  @AC-420.5
  Scenario: Property test — parser handles any report ID value (0x00-0xFF) without panic
    Given any byte value in the range 0x00 to 0xFF as the report ID
    When the parser processes a report with that ID
    Then it SHALL not panic regardless of descriptor configuration

  @AC-420.6
  Scenario: Report ID filtering is covered by unit tests with real HID descriptor samples
    Given HID descriptor samples from real multi-report-ID devices
    When the unit tests are executed
    Then all report ID parsing scenarios SHALL be covered and passing
