@REQ-226 @product
Feature: IL-2 Great Battles UDP adapter provides telemetry and control support  @AC-226.1
  Scenario: IL-2 UDP telemetry parsed from port 34385
    Given the IL-2 Great Battles game is running with telemetry output enabled
    When IL-2 emits UDP packets on port 34385
    Then the adapter SHALL parse those packets and publish telemetry to the bus  @AC-226.2
  Scenario: Aircraft speed altitude and attitude decoded from IL-2 packets
    Given IL-2 UDP packets are being received
    When the adapter processes the packet stream
    Then aircraft speed, altitude, and attitude SHALL be extracted and exposed as telemetry fields  @AC-226.3
  Scenario: Engine health included in telemetry bus snapshot
    Given IL-2 is running with engine telemetry present in packets
    When the adapter publishes a telemetry snapshot
    Then engine RPM and oil temperature SHALL be included in the bus snapshot  @AC-226.4
  Scenario: Gear and flaps state decoded from IL-2 state flags
    Given IL-2 UDP packets include aircraft state flags
    When the adapter decodes the state flags
    Then gear and flaps position SHALL be correctly reflected in the telemetry snapshot  @AC-226.5
  Scenario: IL-2 game pause state reflected in adapter status
    Given the IL-2 adapter is connected and receiving packets
    When the IL-2 game enters the paused state
    Then the adapter status SHALL reflect the paused state without disconnecting  @AC-226.6
  Scenario: Multiple IL-2 aircraft modules supported without per-module config
    Given the IL-2 adapter is configured
    When the user switches between different IL-2 aircraft modules during a session
    Then telemetry SHALL continue working for each module without requiring per-module configuration
