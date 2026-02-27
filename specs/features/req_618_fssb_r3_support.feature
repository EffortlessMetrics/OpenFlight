Feature: FSSB R3 Force Sensing Stick Support
  As a flight simulation enthusiast
  I want OpenFlight to support the FSSB R3 isometric force sensing stick
  So that I can use force-sensing hardware for precise control inputs

  Background:
    Given the OpenFlight service is running

  Scenario: FSSB R3 is identified by its USB descriptor
    When an FSSB R3 device is connected
    Then the service identifies the device using its USB descriptor

  Scenario: Force sensor output is mapped to axis position
    Given the FSSB R3 is connected and identified
    When the user applies force to the stick
    Then the force sensor output is mapped to the corresponding axis position value

  Scenario: Force sensitivity is configurable via FSSB config interface
    When the user adjusts the force sensitivity setting in the FSSB config
    Then the axis response to applied force changes accordingly

  Scenario: FSSB R3 compatibility manifest documents force-to-axis scaling
    When the FSSB R3 compatibility manifest is inspected
    Then it includes documentation of the force-to-axis scaling parameters
