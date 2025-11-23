@REQ-1
Feature: Real-Time Axis Processing

  @AC-1.1
  Scenario: Processing latency under load
    Given a flight-core axis pipeline with 4 axes
    And synthetic telemetry at 250Hz
    When processing 10 minutes of input
    Then p99 latency SHALL be ≤ 5ms

  @AC-1.2
  Scenario: Jitter measurement
    Given a flight-scheduler running at 250Hz
    When measuring tick intervals over 10 minutes
    And excluding the first 5 seconds warm-up
    Then p99 jitter SHALL be ≤ 0.5ms
