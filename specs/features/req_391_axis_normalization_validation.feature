@REQ-391 @product
Feature: Axis Value Normalization Validation — Enforce [-1.0, 1.0] Output Range

  @AC-391.1
  Scenario: Axis pipeline clamps output to [-1.0, 1.0] after final stage
    Given an axis pipeline with multiple processing stages
    When the final stage produces a value outside [-1.0, 1.0]
    Then the output SHALL be clamped to the nearest boundary of [-1.0, 1.0]

  @AC-391.2
  Scenario: NaN or Inf input is replaced with 0.0 and a warning is logged
    Given an axis pipeline receiving input values
    When the input value is NaN or Inf
    Then the value SHALL be replaced with 0.0 and a warning SHALL be logged

  @AC-391.3
  Scenario: Validation runs on the RT thread with zero allocation
    Given the axis normalization validator is active
    When a value is validated on the RT thread
    Then no heap allocation SHALL occur during validation

  @AC-391.4
  Scenario: Validation error count is tracked per-axis and available via metrics
    Given an axis with normalization validation enabled
    When an out-of-range value triggers a validation error
    Then the per-axis validation error counter SHALL be incremented and readable via metrics

  @AC-391.5
  Scenario: Property test — all finite f32 inputs produce output in [-1.0, 1.0]
    Given the axis normalization validator
    When any finite f32 value is passed through the validator
    Then the output SHALL always be in [-1.0, 1.0]

  @AC-391.6
  Scenario: Validation can be disabled per-axis for pass-through use cases
    Given an axis configured for pass-through mode
    When normalization validation is disabled for that axis
    Then values SHALL pass through without clamping or replacement
