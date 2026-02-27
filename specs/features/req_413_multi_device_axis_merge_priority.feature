@REQ-413 @product
Feature: Multi-Device Axis Merge with Priority — Merge Using Device Priority Order

  @AC-413.1
  Scenario: Priority-based merge selects the axis from the highest-priority active device
    Given multiple devices providing the same logical axis
    When a priority-based merge is configured
    Then the output SHALL be the axis value from the highest-priority active device

  @AC-413.2
  Scenario: Priority is numeric with lower values meaning higher priority
    Given devices with priority values assigned
    When two devices are active for the same axis
    Then the device with the numerically lower priority value SHALL take precedence

  @AC-413.3
  Scenario: Fallback to next device when highest-priority device disconnects
    Given a multi-device priority merge active
    When the highest-priority device disconnects
    Then the axis output SHALL fall back to the next highest-priority active device

  @AC-413.4
  Scenario: Priority order is configurable in the profile
    Given a profile with a device priority list for an axis
    When the profile is loaded
    Then the priority order SHALL match what is specified in the profile

  @AC-413.5
  Scenario: Priority change takes effect within one RT tick
    Given a running system with a priority-based axis merge
    When the priority configuration is updated
    Then the new priority order SHALL take effect within one RT tick

  @AC-413.6
  Scenario: Property test — output is always from exactly one device (no averaging)
    Given any number of active devices with a priority merge configured
    When the output is inspected
    Then the value SHALL always originate from exactly one device (not an average)
