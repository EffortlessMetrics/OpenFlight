@REQ-452 @product
Feature: Stick Axis Normalization — Map Raw Hardware Values to -1.0 to 1.0 Range

  @AC-452.1
  Scenario: Raw hardware values are normalized to -1.0 to 1.0
    Given an axis with device min 0 and device max 65535
    When the raw input value is 0
    Then the normalized output SHALL be -1.0
    And when the raw input value is 65535 the normalized output SHALL be 1.0

  @AC-452.2
  Scenario: Center position maps to exactly 0.0
    Given an axis calibrated with center at raw value 32767
    When the raw input equals the center calibration value
    Then the normalized output SHALL be exactly 0.0

  @AC-452.3
  Scenario: NaN and Inf inputs are rejected and logged
    Given a running axis normalization pipeline
    When a NaN or Inf raw value arrives from the hardware layer
    Then the value SHALL be discarded and a warning SHALL be logged with the device identifier

  @AC-452.4
  Scenario: Normalization parameters are loaded from calibration store
    Given a calibration store containing min, center, and max values for an axis
    When the axis normalizer is initialized
    Then it SHALL load parameters from the calibration store rather than using hardware defaults
