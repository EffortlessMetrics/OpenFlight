@REQ-90
Feature: Saitek X52 HID Parsing Property Invariants
  @AC-90.1
  Scenario: WHEN a valid Saitek X52 HID report is parsed THEN all axis values SHALL be within normalized range
    Given the system is configured for REQ-90
    When the feature condition is met
    Then when a valid saitek x52 hid report is parsed then all axis values shall be within normalized range

  @AC-90.2
  Scenario: WHEN a report shorter than minimum length is parsed THEN the result SHALL be an error
    Given the system is configured for REQ-90
    When the feature condition is met
    Then when a report shorter than minimum length is parsed then the result shall be an error

  @AC-90.3
  Scenario: WHEN any arbitrary bytes are parsed THEN axis values SHALL be finite and never NaN or Inf
    Given the system is configured for REQ-90
    When the feature condition is met
    Then when any arbitrary bytes are parsed then axis values shall be finite and never nan or inf

  @AC-90.4
  Scenario: WHEN button bits are extracted THEN they SHALL be within the valid bitmask range
    Given the system is configured for REQ-90
    When the feature condition is met
    Then when button bits are extracted then they shall be within the valid bitmask range
