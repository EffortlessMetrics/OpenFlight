@REQ-22
Feature: Adapter connection resilience and metrics

  @AC-22.1
  Scenario: Reconnection backoff increases exponentially
    Given a reconnection strategy with 100ms initial backoff and 10s maximum
    When backoff delays are calculated for attempts 1, 2, and 3
    Then each subsequent delay SHALL be greater than the previous
    And no delay SHALL exceed the maximum backoff

  @AC-22.1
  Scenario: Retry decision respects max attempts
    Given a reconnection strategy with 3 max attempts
    When should_retry is queried for attempts 1, 2, 3, and 4
    Then attempts 1-3 SHALL return true
    And attempt 4 SHALL return false

  @AC-22.2
  Scenario: Adapter metrics track total updates
    Given a new AdapterMetrics instance
    When multiple telemetry updates are recorded
    Then total_updates SHALL reflect the correct count

  @AC-22.2
  Scenario: Adapter metrics track aircraft changes
    Given a new AdapterMetrics instance
    When an aircraft title change is recorded
    Then aircraft_changes SHALL increment by one
