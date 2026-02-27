@REQ-167 @product
Feature: Profile hot-swap

  @AC-167.1
  Scenario: Aircraft profile loads within 100ms of detection
    Given the system is running with a default profile
    When a new aircraft is detected
    Then the corresponding aircraft profile SHALL be loaded and active within 100 ms

  @AC-167.2
  Scenario: No RT tick missed during profile swap
    Given the RT spine is running at 250 Hz
    When a profile swap is triggered
    Then no RT tick SHALL be missed and no axis output glitch SHALL occur during the swap

  @AC-167.3
  Scenario: Old profile discarded after swap confirms
    Given a profile swap has completed successfully
    When the new profile is confirmed active
    Then the old profile SHALL be discarded from memory

  @AC-167.4
  Scenario: Fallback to default if new profile fails validation
    Given the system is running with an active profile
    When a new profile that fails validation is loaded
    Then the system SHALL fall back to the default profile and emit a validation-failure event

  @AC-167.5
  Scenario: Profile change event published to bus
    Given the event bus is active
    When a profile swap completes
    Then a profile-changed event SHALL be published to the event bus with the new profile identifier

  @AC-167.6
  Scenario: UI notified of profile change
    Given the UI is connected via IPC
    When a profile swap completes
    Then the UI SHALL receive a profile-changed notification over IPC
