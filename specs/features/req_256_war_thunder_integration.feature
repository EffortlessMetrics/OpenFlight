@REQ-256 @product
Feature: War Thunder telemetry adapter provides aircraft state from localhost HTTP  @AC-256.1
  Scenario: War Thunder telemetry JSON polled from localhost
    Given the War Thunder adapter is running
    When the polling interval fires
    Then the adapter SHALL issue an HTTP GET to http://localhost:8111/state and parse the JSON response  @AC-256.2
  Scenario: Altitude airspeed and attitude decoded from War Thunder JSON
    Given a valid War Thunder telemetry JSON response
    When the response is decoded
    Then altitude, airspeed, and attitude fields SHALL be extracted and forwarded to the bus  @AC-256.3
  Scenario: Gear and flaps state decoded from War Thunder JSON
    Given a valid War Thunder telemetry JSON response containing gear and flaps fields
    When the response is decoded
    Then gear-down and flaps-position states SHALL be extracted and published as discrete bus events  @AC-256.4
  Scenario: Vehicle type detected from telemetry
    Given a War Thunder telemetry JSON response
    When the vehicle type field is examined
    Then the adapter SHALL distinguish between aircraft and ground vehicle and set the session vehicle type accordingly  @AC-256.5
  Scenario: Polling rate configurable with default of 20Hz
    Given the War Thunder adapter configuration
    When no polling rate is specified
    Then the adapter SHALL default to 20Hz polling and the rate SHALL be configurable via profile settings  @AC-256.6
  Scenario: Service survives game restart without manual reconnect
    Given the War Thunder adapter is running and the game is restarted
    When the HTTP endpoint becomes available again after the restart
    Then the adapter SHALL resume polling automatically without requiring manual intervention
