@REQ-435 @product
Feature: Axis History Buffer — Circular Buffer of Recent Axis Values in the Axis Engine

  @AC-435.1
  Scenario: History buffer stores last N axis values with N configurable from 64 to 1024
    Given a history buffer capacity of N is set in config
    When the axis engine produces more than N values
    Then only the last N values SHALL be retained in the buffer

  @AC-435.2
  Scenario: Buffer is allocated once at startup and never reallocates
    Given the service has started with a configured history buffer
    When the axis engine runs for many ticks
    Then no heap reallocation SHALL occur for the history buffer after startup

  @AC-435.3
  Scenario: Buffer can be read from non-RT context via snapshot
    Given the axis engine is running
    When a non-RT consumer requests a snapshot
    Then it SHALL receive a copy of the current buffer contents without blocking the RT thread

  @AC-435.4
  Scenario: History snapshot includes timestamps for latency analysis
    Given a history snapshot is taken
    When each record is inspected
    Then it SHALL include a nanosecond-precision timestamp alongside the axis value
