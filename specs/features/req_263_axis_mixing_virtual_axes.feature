@REQ-263 @product
Feature: Axis mixing combines physical axes into a clamped virtual axis  @AC-263.1

  Scenario: Two physical axes summed into a virtual axis
    Given physical axis A reads 0.5 and physical axis B reads 0.3
    When the sum mix is applied to produce virtual axis V
    Then virtual axis V SHALL read 0.8 on the bus

  Scenario: Differential mix of two throttles produces yaw virtual axis
    Given left throttle reads 0.8 and right throttle reads 0.4
    When the differential mix is applied
    Then the yaw virtual axis SHALL read 0.4 (left minus right)  @AC-263.2

  Scenario: Axis scaling factor applied before mix
    Given physical axis A has a scaling factor of 0.5 and reads 1.0 and physical axis B reads 0.0
    When the sum mix is applied
    Then the virtual axis SHALL read 0.5 (scaling applied before summing)  @AC-263.3

  Scenario: Mixed virtual axis output clamped to valid range
    Given physical axis A reads 0.8 and physical axis B reads 0.6 with no scaling
    When the sum mix is applied
    Then the virtual axis output SHALL be clamped to 1.0 and SHALL NOT exceed the valid range  @AC-263.4

  Scenario: Mix configuration is stored and loaded per profile
    Given a profile with a differential mix configuration is saved and then reloaded
    When the profile is applied atomically at a tick boundary
    Then the mix configuration SHALL be active and produce the same output as before save  @AC-263.5

  Scenario: Virtual axis appears in device enumeration API
    Given a virtual axis V has been created via axis mixing
    When the ListDevices gRPC call is made
    Then the response SHALL include virtual axis V with kind "virtual" in the device capability list  @AC-263.6
