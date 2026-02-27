@REQ-431 @product
Feature: SimConnect Axis Injection — Inject Processed Axis Values into MSFS

  @AC-431.1
  Scenario: Adapter sends processed axis values via TransmitClientEvent or SetDataOnSimObject
    Given the SimConnect adapter is connected and injection is enabled
    When the axis engine produces a processed value
    Then the adapter SHALL transmit it via TransmitClientEvent or SetDataOnSimObject as configured

  @AC-431.2
  Scenario: Injection is gated by user-configured enable_injection flag
    Given the profile has enable_injection set to false
    When the axis engine produces values
    Then no injection calls SHALL be made to SimConnect

  @AC-431.3
  Scenario: Injection rate does not exceed sim frame rate
    Given the sim is running at a measured frame rate
    When the adapter queues injection calls
    Then the injection rate SHALL not exceed the sim frame rate

  @AC-431.4
  Scenario: Failed injection calls are logged with SimConnect error codes
    Given a SimConnect call returns a non-SIMCONNECT_EXCEPTION_NONE result
    When the adapter handles the failure
    Then it SHALL log the SimConnect error code and increment an injection_error counter
