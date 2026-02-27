@REQ-68
Feature: Open hardware protocol extended InputReport, LedReport, and FfbOutputReport parsing

  @AC-68.1
  Scenario: InputReport button bitmask is decoded correctly
    Given an InputReport byte buffer with specific button bits set
    When the report is parsed
    Then the button bitmask SHALL reflect the set bits

  @AC-68.1
  Scenario: InputReport hat-switch position is decoded correctly
    Given an InputReport byte buffer with a hat position encoded
    When the report is parsed
    Then the hat-switch position SHALL match the encoded value

  @AC-68.1
  Scenario: InputReport FFB fault flag is detected
    Given an InputReport byte buffer with the FFB fault bit set
    When the report is parsed
    Then the ffb_fault flag SHALL be true

  @AC-68.2
  Scenario: Parsing an empty slice returns None
    Given an empty byte slice
    When InputReport parse is called
    Then it SHALL return None

  @AC-68.2
  Scenario: Parsing a too-short slice returns None
    Given a byte slice shorter than the required report size
    When InputReport parse is called
    Then it SHALL return None

  @AC-68.3
  Scenario: LedReport reserved byte is always zero
    Given a constructed LedReport
    When the reserved byte field is inspected
    Then it SHALL always be zero

  @AC-68.3
  Scenario: LedReport wrong report ID returns None
    Given a byte buffer with an incorrect report ID byte
    When LedReport parse is called
    Then it SHALL return None

  @AC-68.4
  Scenario: FfbOutputReport round-trips for all FFB modes
    Given an FfbOutputReport constructed for each valid FFB mode
    When it is serialized and deserialized
    Then the round-trip SHALL produce the original mode and data

  @AC-68.4
  Scenario: FfbOutputReport with invalid mode byte returns None
    Given a byte buffer with an invalid FFB mode byte
    When FfbOutputReport parse is called
    Then it SHALL return None
