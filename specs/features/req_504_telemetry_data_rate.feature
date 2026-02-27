@REQ-504 @product
Feature: Telemetry Data Rate Monitoring — Per-Adapter Packet Rate Tracking  @AC-504.1
  Scenario: Data rate is measured as packets per second for each adapter
    Given the MSFS adapter is receiving telemetry at 20 Hz
    When the service metrics are sampled
    Then the MSFS adapter data rate SHALL be reported as approximately 20 packets per second  @AC-504.2
  Scenario: Rate below threshold triggers a degraded-mode warning
    Given a telemetry adapter configured with a minimum rate threshold of 10 Hz
    When the measured rate drops to 5 Hz for more than 2 seconds
    Then a degraded-mode warning SHALL be emitted on the service log  @AC-504.3
  Scenario: Rate history is maintained for the last 60 seconds
    Given the service has been running for 90 seconds
    When the rate history for an adapter is queried
    Then the response SHALL contain per-second rate samples covering the last 60 seconds  @AC-504.4
  Scenario: Data rate is included in flightctl status output
    Given the service is running with at least one connected simulator adapter
    When `flightctl status` is executed
    Then the output SHALL include a data rate line for each active adapter
