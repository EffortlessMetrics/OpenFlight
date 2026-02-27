Feature: Bus Subscriber Health Check
  As a flight simulation developer
  I want the bus to detect and remove dead subscribers
  So that stale subscribers do not accumulate and degrade bus performance

  Background:
    Given the OpenFlight service is running

  Scenario: Subscriber health is checked every bus tick via heartbeat
    Given a subscriber is registered on the bus
    When several bus ticks elapse
    Then the bus sends a heartbeat to the subscriber on each tick

  Scenario: Subscribers that miss 3 consecutive heartbeats are removed
    Given a subscriber stops responding to heartbeats
    When 3 consecutive heartbeats are missed
    Then the subscriber is automatically removed from the bus

  Scenario: Subscriber removal is logged with subscriber ID
    Given a dead subscriber is removed
    Then the removal is logged with the subscriber ID

  Scenario: Health check overhead is under 1 microsecond per subscriber
    Given the bus has 100 registered subscribers
    When health checks run for all subscribers in one tick
    Then the total health check time is under 100 microseconds
