Feature: Bus Subscriber Heartbeat
  As a flight simulation enthusiast
  I want bus subscriber heartbeat
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Send periodic heartbeat signals
    Given the system is configured for bus subscriber heartbeat
    When the feature is exercised
    Then bus subscribers send periodic heartbeat signals to indicate liveness

  Scenario: Detect dead subscribers on timeout
    Given the system is configured for bus subscriber heartbeat
    When the feature is exercised
    Then dead subscribers are detected when heartbeat interval is exceeded

  Scenario: Trigger warning on dead subscriber
    Given the system is configured for bus subscriber heartbeat
    When the feature is exercised
    Then dead subscriber detection triggers a warning event on the bus

  Scenario: Configurable heartbeat interval
    Given the system is configured for bus subscriber heartbeat
    When the feature is exercised
    Then heartbeat interval is configurable per subscriber
