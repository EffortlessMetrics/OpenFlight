@REQ-69
Feature: VR overlay extended config validation, notification queue, renderer, and service

  @AC-69.1
  Scenario: OverlayConfig with zero max_notifications is rejected
    Given an OverlayConfig with max_notifications set to zero
    When the config is validated
    Then validation SHALL return an error

  @AC-69.1
  Scenario: OverlayConfig serde round-trip preserves all fields
    Given an OverlayConfig with all fields set
    When it is serialized and deserialized
    Then all fields SHALL be preserved

  @AC-69.2
  Scenario: NotificationQueue push and len track count correctly
    Given an empty NotificationQueue
    When notifications are pushed
    Then the len SHALL equal the number of pushed notifications

  @AC-69.2
  Scenario: NotificationQueue clear empties the queue
    Given a NotificationQueue with notifications
    When clear is called
    Then the queue SHALL be empty

  @AC-69.2
  Scenario: Acknowledging a non-existent notification ID returns false
    Given an empty NotificationQueue
    When acknowledge is called with a non-existent ID
    Then it SHALL return false

  @AC-69.3
  Scenario: Severity display formatting produces expected strings
    Given the Severity variants Info, Warning, and Critical
    When each is formatted for display
    Then the strings SHALL match the expected labels

  @AC-69.3
  Scenario: Severity ordering ranks Critical above Warning above Info
    Given the three Severity variants
    When they are compared by ordering
    Then Critical SHALL rank highest and Info lowest

  @AC-69.4
  Scenario: NullRenderer opacity is clamped to valid range
    Given a NullRenderer
    When opacity is set to a value outside [0.0, 1.0]
    Then the stored opacity SHALL be clamped to the nearest valid boundary

  @AC-69.4
  Scenario: NullRenderer show and hide toggle visibility
    Given a NullRenderer
    When show is called then hide is called
    Then the visible flag SHALL reflect each call

  @AC-69.5
  Scenario: OverlayService spawns and shuts down cleanly
    Given an OverlayService with a NullRenderer
    When the service is spawned and then shut down
    Then it SHALL terminate without error

  @AC-69.5
  Scenario: OverlayService toggle_visible flips visibility
    Given a running OverlayService
    When toggle_visible is called
    Then the visibility state SHALL be inverted

  @AC-69.6
  Scenario: OverlayState default values are correct
    Given a newly created OverlayState
    When its fields are inspected
    Then all fields SHALL have the expected default values

  @AC-69.6
  Scenario: OverlayState serde round-trip preserves all state
    Given an OverlayState with all fields set
    When it is serialized and deserialized
    Then all fields SHALL be preserved
