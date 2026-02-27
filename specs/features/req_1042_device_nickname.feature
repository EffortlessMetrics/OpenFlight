@REQ-1042
Feature: Device Nickname
  @AC-1042.1
  Scenario: Users can assign custom display names to connected devices
    Given the system is configured for REQ-1042
    When the feature condition is met
    Then users can assign custom display names to connected devices

  @AC-1042.2
  Scenario: Nicknames persist across device reconnections using hardware ID
    Given the system is configured for REQ-1042
    When the feature condition is met
    Then nicknames persist across device reconnections using hardware id

  @AC-1042.3
  Scenario: Nicknames are shown in all UI and CLI device listings
    Given the system is configured for REQ-1042
    When the feature condition is met
    Then nicknames are shown in all ui and cli device listings

  @AC-1042.4
  Scenario: Default nickname falls back to manufacturer and product name
    Given the system is configured for REQ-1042
    When the feature condition is met
    Then default nickname falls back to manufacturer and product name
