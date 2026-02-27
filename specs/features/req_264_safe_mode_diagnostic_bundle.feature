@REQ-264 @product
Feature: Safe mode entry produces a structured JSON diagnostic bundle on disk  @AC-264.1

  Scenario: Safe mode entry produces a well-formed JSON bundle
    Given the service is running normally
    When a fault triggers safe mode entry
    Then a diagnostic bundle file SHALL be written and SHALL parse as valid JSON

  Scenario: Bundle includes last 100 log lines
    Given the service has produced more than 100 structured log lines before the fault
    When safe mode is entered
    Then the diagnostic bundle SHALL contain exactly the last 100 log lines in order  @AC-264.2

  Scenario: Bundle includes device enumeration snapshot at fault time
    Given two HID devices are connected at the time of the fault
    When safe mode is entered
    Then the diagnostic bundle SHALL include a device snapshot listing both devices with VID and PID  @AC-264.3

  Scenario: Bundle includes the active profile at fault time
    Given a named profile "aerobatics" is active when the fault occurs
    When safe mode is entered
    Then the diagnostic bundle SHALL include the serialised "aerobatics" profile in full  @AC-264.4

  Scenario: Bundle includes axis values at the moment of fault
    Given axis "aileron" reads 0.3 and axis "elevator" reads -0.1 at the moment of fault
    When safe mode is entered
    Then the diagnostic bundle SHALL record aileron as 0.3 and elevator as -0.1  @AC-264.5

  Scenario: Bundle is written atomically to disk on safe mode entry
    Given the service is configured with a diagnostics output directory
    When safe mode is entered
    Then the bundle file SHALL appear in the diagnostics directory as a completed write with no partial file  @AC-264.6
