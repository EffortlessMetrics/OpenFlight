@REQ-367 @product
Feature: Axis Smoothing Windowed Moving Average

  @AC-367.1
  Scenario: Window size is configurable per axis
    Given an axis with a windowed moving average filter configured
    When the window size is set to a value between 2 and 64
    Then the axis SHALL use that window size for the averaging computation

  @AC-367.2
  Scenario: Window of 1 passes input unchanged
    Given an axis with a windowed moving average filter with window size 1
    When an input value is processed
    Then the output SHALL equal the input value unchanged

  @AC-367.3
  Scenario: Buffer is fully pre-allocated
    Given an axis with a windowed moving average filter initialised
    When the filter processes values at RT rate
    Then no heap allocation SHALL occur during processing

  @AC-367.4
  Scenario: Output is bounded by window contents
    Given an axis with a windowed moving average filter with window size 8
    When a sequence of input values is processed
    Then each output value SHALL be within the min and max of the current window contents

  @AC-367.5
  Scenario: Latency is reported as half the window size
    Given an axis with a windowed moving average filter with window size W
    When the filter reports its introduced latency
    Then the reported latency SHALL equal W divided by 2 samples

  @AC-367.6
  Scenario: Property test — output never exceeds bounds of inputs seen in window
    Given an axis with a windowed moving average filter
    When arbitrary input sequences are processed via property testing
    Then the output SHALL never exceed the min or max of the inputs present in the current window
