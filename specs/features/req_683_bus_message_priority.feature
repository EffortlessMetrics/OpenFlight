Feature: Bus Message Priority
  As a flight simulation enthusiast
  I want event bus messages to support priority levels
  So that critical messages are processed promptly even under load

  Background:
    Given the OpenFlight service is running

  Scenario: Messages can be tagged with priority levels
    Given the event bus is active
    When a message is published with High, Normal, or Low priority
    Then the message is accepted and tagged with the specified priority level

  Scenario: High priority messages are processed before Normal
    Given the event bus has queued Normal and High priority messages simultaneously
    When the bus processes messages
    Then all High priority messages are delivered before Normal priority messages

  Scenario: Low priority messages are dropped on queue pressure
    Given the event bus queue is under pressure and at capacity
    When a Low priority message is published
    Then the Low priority message is dropped to relieve queue pressure

  Scenario: Priority levels are configurable in service config
    Given the service configuration file is updated with custom priority thresholds
    When the service is restarted
    Then the bus applies the configured priority levels
