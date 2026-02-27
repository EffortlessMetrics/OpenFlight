@REQ-442 @product
Feature: Axis Bypass Mode — Raw Hardware Value Passthrough in the Axis Engine

  @AC-442.1
  Scenario: Bypass mode sends raw hardware values without any processing
    Given an axis configured with curves, deadzones, and trims
    When bypass mode is enabled for that axis
    Then the axis engine SHALL pass the raw hardware value directly to the output

  @AC-442.2
  Scenario: Bypass is configurable per axis
    Given a multi-axis device
    When bypass is enabled on one axis
    Then only that axis SHALL be in bypass; other axes SHALL continue normal processing

  @AC-442.3
  Scenario: Switching to bypass does not cause value jumps
    Given an axis in normal processing mode producing a steady output
    When bypass is enabled mid-operation
    Then the output value transition SHALL not produce an instantaneous jump larger than one raw step

  @AC-442.4
  Scenario: Bypass state is reported in axis health metrics
    Given bypass mode is active on an axis
    When axis health metrics are queried
    Then the metrics SHALL include a bypass_active flag set to true for that axis
