@REQ-356 @product
Feature: Simulator Connection Health Dashboard  @AC-356.1
  Scenario: Each sim adapter reports health fields
    Given a sim adapter for MSFS is connected
    When the health data is queried
    Then the response SHALL include connected, last_rx_at, packets_per_second, and error_count  @AC-356.2
  Scenario: Health metrics are accessible via /metrics HTTP endpoint
    Given the service is running
    When an HTTP GET request is made to /metrics
    Then the response SHALL include per-simulator connection health data  @AC-356.3
  Scenario: Stale metrics are flagged
    Given a sim adapter has not sent an update for more than 5 seconds
    When the health data is queried
    Then the response SHALL include stale: true for that adapter  @AC-356.4
  Scenario: Dashboard shows connection state transitions with timestamps
    Given a sim adapter transitions from disconnected to connected
    When the health data is queried
    Then the response SHALL include the transition timestamp and new state  @AC-356.5
  Scenario: Disconnect events increment the sim_disconnects_total counter
    Given a sim adapter was previously connected
    When the sim adapter disconnects
    Then the sim_disconnects_total counter for that adapter SHALL be incremented by 1  @AC-356.6
  Scenario: Health data is updated at minimum every 1 second
    Given a sim adapter is connected and sending data
    When 1 second elapses
    Then the health metrics SHALL reflect an updated last_rx_at within the last second
