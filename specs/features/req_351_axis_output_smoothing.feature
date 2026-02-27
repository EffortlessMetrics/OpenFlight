@REQ-351 @axis @smoothing @rt-core
Feature: Axis output smoothing with configurable window
  As a user configuring axis processing
  I want windowed moving average smoothing distinct from EMA
  So that I can reduce impulse noise with a predictable delay budget

  Scenario: Window size is configurable per axis  @AC-351.1
    Given an axis is configured with window size 8
    When the configuration is applied
    Then the smoothing filter SHALL use an 8-sample moving average

  Scenario: Windowed average reduces impulse noise  @AC-351.2
    Given an axis is configured with window size 16
    When a single spike of 1.0 is injected into a stream of 0.0 values
    Then the output SHALL never exceed 1.0 / 16

  Scenario: Window size of 1 disables smoothing  @AC-351.3
    Given an axis is configured with window size 1
    When any input value is provided
    Then the output SHALL equal the input without modification

  Scenario: Buffer is pre-allocated with no runtime heap use  @AC-351.4
    Given the smoothing filter is initialized with window size 32
    When 1000 samples are processed
    Then no heap allocation SHALL occur during sample processing

  Scenario: Smoothing latency scales with window size  @AC-351.5
    Given an axis with window size W receives a step input
    When the steady-state output is measured
    Then the latency SHALL be approximately W / 2 samples

  Scenario: Property test - output stays within input bounds  @AC-351.6
    Given an axis with window size between 1 and 64
    When arbitrary input values in [-1.0, 1.0] are fed to the filter
    Then all outputs SHALL remain within [-1.0, 1.0]
