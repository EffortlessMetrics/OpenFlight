@REQ-339 @product
Feature: Analog Panel Support  @AC-339.1
  Scenario: Service reads analog inputs from dedicated panel ADC devices
    Given a dedicated analog panel device with ADC inputs is connected
    When the service starts
    Then the service SHALL successfully read analog input values from the panel device  @AC-339.2
  Scenario: Analog panel axes are treated as configurable axis sources
    Given an analog panel device is connected and recognised
    When configuring an axis mapping in a profile
    Then the analog panel inputs SHALL be available as selectable axis sources alongside joystick axes  @AC-339.3
  Scenario: Panel VID/PID is listed in the compatibility matrix
    Given the project compatibility matrix document
    When inspecting the list of supported devices
    Then the analog panel device VID and PID SHALL appear in the compatibility matrix  @AC-339.4
  Scenario: Analog resolution is configurable
    Given a profile that specifies 12-bit resolution for an analog panel axis
    When the axis value is read from the device
    Then the service SHALL interpret the raw value with 12-bit precision  @AC-339.5
  Scenario: Panel calibration follows the joystick calibration flow
    Given an analog panel device that requires calibration
    When the user initiates calibration via "flightctl calibrate <device>"
    Then the calibration wizard SHALL present the same min/center/max capture steps used for joystick calibration  @AC-339.6
  Scenario: Panel disconnect is handled without axis spike
    Given an analog panel axis mapped to a flight control
    When the panel device is disconnected unexpectedly
    Then the axis value SHALL be held at its last known value and no spike SHALL be emitted
