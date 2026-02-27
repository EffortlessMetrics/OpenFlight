@REQ-509 @product
Feature: Cross-Platform Compatibility Testing — Core Tests on Windows and Linux  @AC-509.1
  Scenario: CI runs core crate tests on both Windows and Linux runners
    Given the CI pipeline is defined with matrix runners
    When a pull request targets main
    Then core crate tests SHALL execute on both a Windows runner and a Linux runner  @AC-509.2
  Scenario: Platform-specific code is properly gated with cfg attributes
    Given the source code for platform-specific subsystems
    When a static analysis pass checks cfg gating
    Then all platform-specific code SHALL be wrapped in appropriate cfg(target_os) attributes  @AC-509.3
  Scenario: Hardware-specific tests skip gracefully on platforms without hardware
    Given a test that requires physical HID hardware
    When the test runs on a CI runner without HID hardware attached
    Then the test SHALL be skipped with a descriptive reason rather than failing  @AC-509.4
  Scenario: Cross-platform test coverage is tracked in CI
    Given CI runs on both Windows and Linux
    When the CI summary is produced
    Then the summary SHALL include a cross-platform coverage section listing tests run per platform
