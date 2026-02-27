@REQ-396 @product
Feature: SimConnect Connection Retry with Exponential Backoff

  @AC-396.1
  Scenario: Connection failure triggers exponential backoff retry sequence
    Given a SimConnect adapter that cannot connect
    When connection attempts fail
    Then retry intervals SHALL follow the sequence 1s, 2s, 4s, 8s, 16s, 32s

  @AC-396.2
  Scenario: Maximum retry interval is capped at 60 seconds
    Given a SimConnect adapter in backoff retry mode
    When the backoff interval would exceed 60 seconds
    Then the interval SHALL be capped at 60 seconds

  @AC-396.3
  Scenario: Backoff state is reset on successful connection
    Given a SimConnect adapter that was in backoff retry mode
    When a connection attempt succeeds
    Then the backoff state and retry counter SHALL be reset

  @AC-396.4
  Scenario: Retry count and next retry time are available via adapter metrics
    Given a SimConnect adapter in backoff retry mode
    When adapter metrics are queried
    Then the retry count and next retry timestamp SHALL be reported

  @AC-396.5
  Scenario: Retry can be interrupted immediately via flightctl
    Given a SimConnect adapter waiting for the next retry
    When the user runs `flightctl simconnect reconnect`
    Then a connection attempt SHALL be initiated immediately

  @AC-396.6
  Scenario: Integration test verifies backoff timing with mock connection failures
    Given a mock SimConnect endpoint that rejects connections
    When the adapter attempts to connect multiple times
    Then the observed retry intervals SHALL match the expected backoff sequence
