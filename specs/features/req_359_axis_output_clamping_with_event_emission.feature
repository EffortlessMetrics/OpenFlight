@REQ-359 @product
Feature: Axis Output Clamping with Event Emission  @AC-359.1
  Scenario: Axis output is clamped to configured min/max
    Given an axis with min configured to -1.0 and max configured to 1.0
    When the computed output value is 1.5
    Then the output SHALL be clamped to 1.0  @AC-359.2
  Scenario: A ClampEvent is emitted when output exceeds bounds
    Given an axis with max configured to 1.0
    When the computed output value is 1.2
    Then a ClampEvent SHALL be emitted for that axis  @AC-359.3
  Scenario: ClampEvent contains required fields
    Given a ClampEvent is emitted for axis "throttle" with unclamped value 1.3 and clamped value 1.0
    When the event is inspected
    Then it SHALL contain axis_id "throttle", unclamped_value 1.3, clamped_value 1.0, and a timestamp  @AC-359.4
  Scenario: Clamp event count is tracked per-axis via metrics
    Given the throttle axis has been clamped 5 times
    When the metrics endpoint is queried
    Then the clamp_event_total counter for the throttle axis SHALL equal 5  @AC-359.5
  Scenario: Clamping can be disabled per-axis in pass-through mode
    Given an axis configured with clamping disabled
    When the computed output value is 1.5
    Then the output SHALL be 1.5 without clamping  @AC-359.6
  Scenario: Clamp events do not allocate on the RT thread
    Given clamping is enabled for an axis on the RT spine
    When clamping occurs during a 250 Hz tick
    Then no heap allocation SHALL occur on the RT thread during the clamp event emission
