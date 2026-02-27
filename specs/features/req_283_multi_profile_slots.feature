@REQ-283 @product
Feature: Multi-profile slots with atomic switching, slot persistence, and StreamDeck integration  @AC-283.1
  Scenario: User can define up to 8 named profile slots
    Given a profile configuration file
    When the user defines 8 named slots each with a distinct name and axis configuration
    Then all 8 slots SHALL be accepted and stored without error  @AC-283.2
  Scenario: Active slot can be switched via CLI in under 50ms
    Given the service is running with multiple profile slots loaded
    When the user invokes the CLI slot switch command targeting a different slot
    Then the active slot SHALL change and the switch SHALL complete in under 50 milliseconds  @AC-283.3
  Scenario: Slot switch is atomic with no partial application
    Given the service is running and an axis tick is in progress
    When a slot switch command arrives concurrently with the running tick
    Then the new slot configuration SHALL be applied atomically at the next tick boundary with no partial state visible  @AC-283.4
  Scenario: Each slot stores independent axis curves and deadzones
    Given two profile slots each configured with different axis curves and deadzone values
    When the active slot is switched between them
    Then the axis pipeline SHALL use the curves and deadzones of the newly active slot exclusively  @AC-283.5
  Scenario: Slot assignments are persisted across service restarts
    Given the user has switched to slot 3 and the service is then stopped and restarted
    When the service completes startup
    Then slot 3 SHALL remain the active slot without requiring the user to re-select it  @AC-283.6
  Scenario: StreamDeck button can trigger slot switch
    Given a StreamDeck button is bound to a profile slot switch action for slot 2
    When the user presses that StreamDeck button
    Then the service SHALL switch the active profile slot to slot 2 within one processing cycle
