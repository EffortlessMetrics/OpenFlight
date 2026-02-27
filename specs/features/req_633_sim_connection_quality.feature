Feature: Sim Connection Quality Metric
  As a flight simulation enthusiast
  I want the service to track and report sim connection quality
  So that I can diagnose and monitor the health of my sim connection

  Background:
    Given the OpenFlight service is running

  Scenario: Connection quality score ranges from 0 to 100
    Given a simulator is connected
    When the connection quality score is retrieved
    Then the score value is between 0 and 100 inclusive

  Scenario: Score reflects packet loss, latency, and reconnect count
    Given a simulator connection with known loss and latency is active
    When the quality score is computed
    Then the score accounts for packet loss, round-trip latency, and reconnect count

  Scenario: Quality score is available via gRPC diagnostics RPC
    Given the service is running
    When the diagnostics gRPC RPC is called
    Then the response includes the current connection quality score

  Scenario: Score dropping below threshold triggers a warning event
    Given the quality threshold is configured
    When the connection quality score drops below the threshold
    Then a warning event is emitted on the event bus
