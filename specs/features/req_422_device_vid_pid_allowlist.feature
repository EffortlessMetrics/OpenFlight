@REQ-422 @product
Feature: Device VID/PID Allow-List — Restrict Which Devices Can Be Used

  @AC-422.1
  Scenario: Config allows specifying an allow-list of VID/PID pairs
    Given a service configuration
    When a vid_pid_allowlist section is present with one or more VID/PID entries
    Then only those VID/PID pairs SHALL be eligible for HID enumeration

  @AC-422.2
  Scenario: Devices not on the allow-list are excluded from HID enumeration
    Given an allow-list with specific VID/PID entries
    When the HID subsystem enumerates devices
    Then any device whose VID/PID is not on the list SHALL be excluded

  @AC-422.3
  Scenario: Empty allow-list means all devices are allowed (default behavior)
    Given a service configuration with an empty or absent vid_pid_allowlist
    When HID enumeration runs
    Then all detected HID devices SHALL be considered (default behavior preserved)

  @AC-422.4
  Scenario: Blocked devices are logged at DEBUG level
    Given an allow-list that excludes some connected devices
    When enumeration runs and a device is excluded
    Then a DEBUG-level log entry SHALL be emitted identifying the blocked device

  @AC-422.5
  Scenario: flightctl devices list --all shows blocked devices with a blocked indicator
    Given connected devices some of which are blocked by the allow-list
    When `flightctl devices list --all` is executed
    Then blocked devices SHALL appear in the output with a visual indicator (e.g. 🚫)

  @AC-422.6
  Scenario: Allow-list changes take effect on next service restart
    Given an allow-list that is modified while the service is running
    When the service is restarted
    Then the updated allow-list SHALL be applied during the next enumeration pass
