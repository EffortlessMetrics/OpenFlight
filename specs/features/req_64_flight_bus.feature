@REQ-64
Feature: Event bus, pub/sub, and bus snapshots

  @AC-64.1
  Scenario: Subscriber receives a published snapshot
    Given a BusPublisher with one subscriber
    When a telemetry snapshot is published
    Then the subscriber's try_recv SHALL return the snapshot

  @AC-64.2
  Scenario: Multiple subscribers each receive the same snapshot
    Given a BusPublisher with three subscribers
    When a single snapshot is published
    Then each subscriber SHALL independently receive that snapshot via try_recv

  @AC-64.3
  Scenario: Late subscriber receives no stale data
    Given a BusPublisher that has already published several snapshots
    When a new subscriber is created after all publications
    Then the new subscriber's try_recv SHALL return None

  @AC-64.4
  Scenario: Publishing a snapshot with NaN values is rejected
    Given a BusPublisher
    When a snapshot containing a NaN angular rate is published
    Then the publish call SHALL return an error and no subscriber SHALL receive data

  @AC-64.5
  Scenario: Dropped subscriber is cleaned up on next publish
    Given a BusPublisher with one subscriber that is subsequently dropped
    When the next snapshot is published
    Then the publisher's internal subscriber list SHALL no longer contain the dead handle
