@REQ-357 @product
Feature: Falcon BMS Integration  @AC-357.1
  Scenario: Falcon BMS shared memory is opened on sim start
    Given Falcon BMS is running with shared memory segments FlightData, FlightData2, and OSBData
    When the Falcon BMS adapter initialises
    Then all three shared memory segments SHALL be opened successfully  @AC-357.2
  Scenario: Axis positions are read from shared memory
    Given the FlightData shared memory is open and contains pilot axis data
    When the adapter reads a frame
    Then pitch, roll, yaw, and throttle values SHALL be extracted from shared memory  @AC-357.3
  Scenario: Data is read at 30 Hz minimum
    Given the Falcon BMS adapter is running
    When 1 second elapses
    Then the adapter SHALL have performed at least 30 shared-memory read cycles  @AC-357.4
  Scenario: Shared memory handle is closed cleanly on shutdown
    Given the Falcon BMS adapter has open shared memory handles
    When the service is stopped
    Then all shared memory handles SHALL be closed without errors  @AC-357.5
  Scenario: Missing shared memory produces graceful degradation
    Given Falcon BMS is not running and its shared memory does not exist
    When the Falcon BMS adapter attempts to initialise
    Then the adapter SHALL log a warning and operate in degraded mode without crashing  @AC-357.6
  Scenario: BMS version is detected and logged at startup
    Given Falcon BMS shared memory is available
    When the adapter initialises
    Then the detected BMS version string SHALL be logged at info level
