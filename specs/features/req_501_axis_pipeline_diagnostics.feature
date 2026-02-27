@REQ-501 @product
Feature: Axis Pipeline Diagnostics — Per-Stage Values Exposed for Inspection  @AC-501.1
  Scenario: Each pipeline stage output is recorded in ChainStageValues struct
    Given an axis pipeline with deadzone, curve, and trim stages active
    When the axis processes an input value
    Then the ChainStageValues struct SHALL contain an output value for each stage  @AC-501.2
  Scenario: Stage values are accessible via IPC diagnostic query
    Given the service is running with diagnostics enabled
    When a client sends an IPC diagnostic query for a specific axis
    Then the response SHALL include the ChainStageValues for the most recent tick  @AC-501.3
  Scenario: Stage diagnostics include input post-deadzone post-curve and output values
    Given an axis pipeline diagnostic response
    When the stage values are inspected
    Then the response SHALL contain fields for input, post-deadzone, post-curve, and final output  @AC-501.4
  Scenario: Diagnostics collection has near-zero overhead when disabled
    Given the axis pipeline with diagnostics disabled
    When the axis processes values at 250 Hz for one second
    Then the processing time SHALL not exceed the baseline by more than 1 microsecond per tick
