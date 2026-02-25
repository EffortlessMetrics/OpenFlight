# REQ-48 VR Overlay
# Covers: configuration, notification queue, state management, renderer abstraction, service lifecycle

Feature: VR Overlay

  Background:
    Given the VR overlay is initialised with a NullRenderer

  # ── Configuration ─────────────────────────────────────────────────────────

  @AC-48.1
  Scenario: Default configuration validates successfully
    Given OverlayConfig::default() is used
    Then config.validate() returns Ok

  @AC-48.1
  Scenario: Opacity out of range is rejected
    Given an OverlayConfig with opacity=1.5
    Then config.validate() returns an error containing "opacity"

  @AC-48.2
  Scenario: Zero scale is rejected
    Given an OverlayConfig with scale=0.0
    Then config.validate() returns an error containing "scale"

  @AC-48.2
  Scenario: Depth too small is rejected
    Given an OverlayConfig with depth_m=0.05
    Then config.validate() returns an error containing "depth_m"

  # ── Notifications ──────────────────────────────────────────────────────────

  @AC-48.3
  Scenario: Push a notification and check queue length
    When I push a Severity::Info notification "Profile loaded" with TTL=60s
    Then the queue length is 1

  @AC-48.3
  Scenario: Expired notification is pruned
    Given a notification with TTL=1ms is pushed
    When I wait 5ms and call prune_expired()
    Then the queue is empty

  @AC-48.4
  Scenario: Queue at capacity evicts oldest non-Critical item
    Given the queue capacity is 2
    And I push Info "first" and Warning "second"
    When I push Info "third"
    Then "first" is no longer in the queue
    And "third" is in the queue

  @AC-48.4
  Scenario: Queue at capacity drops incoming when all slots are Critical
    Given the queue capacity is 2
    And both slots hold Critical notifications
    When I push an Info notification
    Then the Info notification is not in the queue

  @AC-48.5
  Scenario: Acknowledge marks notification as expired
    Given a persistent notification "ack-me" is in the queue
    When I acknowledge "ack-me"
    Then the notification is flagged as acknowledged and expired

  @AC-48.5
  Scenario: max_severity returns highest severity in active notifications
    Given the queue contains Info and Warning notifications
    Then max_severity() returns Warning

  @AC-48.6
  Scenario: Notifications sorted by severity descending in active()
    Given the queue contains Info, Critical, and Warning notifications
    Then active()[0].severity is Critical

  # ── State ─────────────────────────────────────────────────────────────────

  @AC-48.7
  Scenario: Toggle visibility flips visible flag
    Given the overlay state has visible=true
    When toggle_visible() is called
    Then visible is false

  @AC-48.7
  Scenario: axes_in_deadzone counts only deadzone axes
    Given the overlay state has 3 axes: Roll(in_deadzone=true), Pitch(false), Throttle(true)
    Then axes_in_deadzone() returns 2

  # ── Service lifecycle ─────────────────────────────────────────────────────

  @AC-48.8
  Scenario: Spawn and shutdown service
    When OverlayService::spawn() is called with a NullRenderer
    And shutdown() is called on the handle
    Then the service terminates cleanly

  @AC-48.9
  Scenario: SetProfile command updates state
    Given the overlay service is running
    When I send SetProfile("MSFS-747")
    Then handle.state().profile_name is "MSFS-747"
