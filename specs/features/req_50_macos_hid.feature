# REQ-50: macOS HID Device Layer

Feature: macOS IOKit/HID device enumeration and report I/O (scaffold)

  Background:
    Given the flight-macos-hid crate is compiled on the current platform

  # ─── Platform guard (non-macOS) ─────────────────────────────────────────────

  Scenario: HidManager creation succeeds on non-macOS as a stub
    Given the platform is not macOS
    When HidManager::new() is called
    Then it should return Ok with an empty device list

  Scenario: HidManager open returns UnsupportedPlatform on non-macOS
    Given the platform is not macOS
    And a HidManager has been created
    When open() is called
    Then it should return Err(HidError::UnsupportedPlatform)

  Scenario: HidDevice open returns UnsupportedPlatform on non-macOS
    Given the platform is not macOS
    And a HidDeviceInfo for VID 0x044F PID 0xB67B
    When HidDevice::open() is called
    Then it should return Err(HidError::UnsupportedPlatform)

  # ─── Device matching criteria ────────────────────────────────────────────────

  Scenario: set_device_matching stores usage page and usage
    Given a new HidManager
    When set_device_matching(0x01, 0x04) is called
    Then criteria().usage_page should be Some(0x01)
    And criteria().usage should be Some(0x04)

  Scenario: set_vendor_product stores VID and PID
    Given a new HidManager
    When set_vendor_product(0x044F, 0xB67B) is called
    Then criteria().vendor_id should be Some(0x044F)
    And criteria().product_id should be Some(0xB67B)

  # ─── Timing / MacosClock ─────────────────────────────────────────────────────

  Scenario: MacosClock elapsed increases over time on non-macOS
    Given a MacosClock is created
    When 1 millisecond passes
    Then elapsed() should be at least 1ms

  Scenario: MacosClock now_ns is monotonically non-decreasing
    Given a MacosClock is created
    When now_ns() is sampled twice
    Then the second sample should be >= the first

  # ─── Error display ───────────────────────────────────────────────────────────

  Scenario: UnsupportedPlatform error has descriptive message
    When HidError::UnsupportedPlatform is formatted as a string
    Then the output should contain "not supported"

  Scenario: OpenFailed error includes the IOKit return code
    When HidError::OpenFailed { code: -0x1FFF_FD3B } is formatted
    Then the output should contain the hex code

  # ─── macOS IOKit (stub — compile-only guard) ─────────────────────────────────

  Scenario: IOKit paths are guarded behind cfg(target_os = "macos")
    Given the crate is compiled on a non-macOS platform
    Then no IOKit symbols are required at link time
    And the crate compiles successfully

  Scenario: Workspace IOKit dependencies are target-conditional
    Given the Cargo.toml for flight-macos-hid
    Then IOKit dependencies appear only under [target.'cfg(target_os = "macos")'.dependencies]
    And no IOKit crates are resolved on Windows or Linux builds
