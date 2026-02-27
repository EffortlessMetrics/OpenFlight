@REQ-129 @infra
Feature: Tracing and observability  @AC-129.1
  Scenario: TraceEvent records message and severity level
    Given a tracing subsystem initialized with INFO level
    When a TraceEvent is emitted with message "axis processed" and severity INFO
    Then the event SHALL be stored with the exact message and severity level  @AC-129.2
  Scenario: Span has positive duration
    Given a tracing span is opened at time T1
    When the span is closed at time T2 where T2 > T1
    Then the recorded span duration SHALL be positive and equal to T2 minus T1  @AC-129.3
  Scenario: Buffer cap limits retained samples
    Given a trace buffer with a capacity of 1000 samples
    When 1500 events are emitted
    Then the buffer SHALL retain at most 1000 samples
    And the oldest samples SHALL be dropped to enforce the cap  @AC-129.4
  Scenario: JSON export produces valid structure
    Given a trace buffer containing at least one event
    When the buffer is exported as JSON
    Then the resulting document SHALL be valid JSON
    And each entry SHALL contain a timestamp, severity, and message field  @AC-129.5
  Scenario: Log level filtering excludes debug in production
    Given the tracing subsystem configured at INFO level
    When a DEBUG event is emitted
    Then the event SHALL NOT be stored in the buffer  @AC-129.6
  Scenario: Concurrent writes don't corrupt trace
    Given eight threads each emitting 500 trace events concurrently
    When all threads complete
    Then no event in the buffer SHALL have a corrupted message or invalid severity
    And the total retained count SHALL be at most the buffer capacity  @AC-129.7
  Scenario: Reset clears all counters and samples
    Given a trace buffer containing 200 events and non-zero counters
    When a reset is issued
    Then the buffer SHALL contain zero events
    And all counters SHALL be zero  @AC-129.8
  Scenario: HID write latency event has 25-byte fixed format
    Given the HID write latency event type is emitted
    When the event is serialised to its wire format
    Then the resulting byte slice SHALL be exactly 25 bytes long
