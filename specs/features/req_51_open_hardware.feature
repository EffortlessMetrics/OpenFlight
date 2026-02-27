# REQ-51: Open Hardware Reference FFB Stick Protocol

Feature: OpenFlight reference hardware — HID protocol definitions

  Background:
    Given the flight-open-hardware crate is compiled (no_std)

  # ─── Input Report 0x01 ────────────────────────────────────────────────────────

  @AC-51.1
  Scenario: Parse a centred input report
    Given a 16-byte input report buffer with report ID 0x01 and all axis bytes zero
    When InputReport::parse() is called
    Then x, y, twist should be 0
    And throttle should be 0
    And no buttons should be active
    And ffb_fault should be false

  @AC-51.2
  Scenario: Roundtrip serialise and parse an input report
    Given an InputReport with x=16383, y=-16383, throttle=128, buttons=0b10100101, hat=3
    When to_bytes() is called and the result is parsed
    Then the parsed report should equal the original

  @AC-51.3
  Scenario: Wrong report ID returns None on parse
    Given a 16-byte buffer with first byte 0x99
    When InputReport::parse() is called
    Then the result should be None

  @AC-51.3
  Scenario: Buffer too short returns None on parse
    Given a 4-byte buffer with first byte 0x01
    When InputReport::parse() is called
    Then the result should be None

  @AC-51.4
  Scenario: Normalise full-deflection axes to [-1, 1]
    Given an InputReport with x=32767 and y=-32767 and throttle=255
    When x_norm(), y_norm(), throttle_norm() are called
    Then x_norm should be approximately 1.0
    And y_norm should be approximately -1.0
    And throttle_norm should be approximately 1.0

  # ─── FFB Output Report 0x10 ─────────────────────────────────────────────────

  @AC-51.5
  Scenario: Stop report serialises correctly
    When FfbOutputReport::stop() is called
    Then the first byte should be 0x10
    And force_x and force_y bytes should be zero
    And mode byte should be 0 (Off)

  @AC-51.6
  Scenario: Roundtrip FFB output report
    Given an FfbOutputReport with force_x=-16000, force_y=16000, mode=Spring, gain=200
    When to_bytes() is called and the result is parsed
    Then the parsed report should equal the original

  @AC-51.3
  Scenario: Wrong report ID returns None for FFB parse
    Given an 8-byte buffer with first byte 0xFF
    When FfbOutputReport::parse() is called
    Then the result should be None

  # ─── LED Report 0x20 ─────────────────────────────────────────────────────────

  @AC-51.7
  Scenario: All-off LED report has zero LED flags
    When LedReport::all_off() is called
    Then the first byte should be 0x20
    And the leds byte should be 0

  @AC-51.7
  Scenario: Roundtrip LED report with PC_MODE and POWER flags
    Given a LedReport with leds=(POWER | PC_MODE) and brightness=200
    When to_bytes() is called and the result is parsed
    Then the parsed report should equal the original

  # ─── Firmware Version Report 0xF0 ────────────────────────────────────────────

  @AC-51.8
  Scenario: Roundtrip firmware version report
    Given a FirmwareVersionReport with major=1, minor=2, patch=3, hash=[0xDE,0xAD,0xBE,0xEF]
    When to_bytes() is called and the result is parsed
    Then version() should return (1, 2, 3)

  @AC-51.3
  Scenario: Wrong report ID returns None for firmware version parse
    Given an 8-byte buffer with first byte 0x01
    When FirmwareVersionReport::parse() is called
    Then the result should be None

  # ─── Protocol constants ───────────────────────────────────────────────────────

  @AC-51.9
  Scenario: USB identifiers match reference design spec
    Then VENDOR_ID should be 0x1209 (pid.codes open allocation)
    And PRODUCT_ID should be 0xF170

  @AC-51.9
  Scenario: Crate compiles in no_std environment
    Given the Cargo.toml for flight-open-hardware has no std dependencies
    Then the crate should compile with #![no_std]
