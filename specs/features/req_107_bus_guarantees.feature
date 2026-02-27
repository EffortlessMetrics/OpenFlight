@REQ-107 @product
Feature: Event bus delivery guarantees for flight-bus

  @AC-107.1
  Scenario: Published message reaches all active subscribers
    Given a BusPublisher with multiple active subscribers
    When a single valid snapshot is published
    Then each subscriber SHALL independently receive the snapshot via try_recv

  @AC-107.2
  Scenario: FIFO ordering is preserved within a channel
    Given a BusPublisher with one subscriber
    When N snapshots are published in sequence with distinct SimId tags
    Then the subscriber SHALL receive all N snapshots in exactly that order

  @AC-107.3
  Scenario: Rate-limited drop reaches no subscriber (all-or-nothing)
    Given a BusPublisher with N subscribers (1 to 4) and the rate limiter fires
    When a back-to-back second snapshot is published before the minimum interval
    Then none of the N subscribers SHALL receive the rate-limited snapshot

  @AC-107.4
  Scenario: Arbitrary float payloads including NaN and Inf never cause a panic
    Given a BusPublisher and a snapshot whose float fields carry any f32 bit pattern
    When the snapshot is published
    Then the call SHALL return Ok or Err but SHALL NOT panic

  @AC-107.5
  Scenario: Subscriber rate limit prevents delivery without blocking the publisher
    Given a BusPublisher and a subscriber configured with a low per-subscriber max_rate_hz
    When the publisher publishes a second snapshot before the subscriber's minimum interval
    Then the subscriber SHALL not receive the second snapshot but the publish call SHALL succeed
