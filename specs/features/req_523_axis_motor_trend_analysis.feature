@REQ-523 @product
Feature: Axis Motor Trend Analysis — Adaptive Smoothing Based on Motion Trends  @AC-523.1
  Scenario: Trend detector identifies rapid input changes
    Given the axis engine is processing input at 250Hz
    When a large input delta is detected across consecutive samples
    Then the trend detector SHALL classify the motion as rapid change  @AC-523.2
  Scenario: Adaptive smoothing increases filtering during slow drift
    Given the trend detector has classified motion as slow drift
    When the axis filter chain is applied
    Then the smoothing coefficient SHALL be increased above the baseline value  @AC-523.3
  Scenario: Trend analysis window size is configurable
    Given a profile specifies a trend window of 8 samples
    When the axis engine initialises the trend detector
    Then the detector SHALL use exactly 8 samples to compute the trend state  @AC-523.4
  Scenario: Trend state is exposed in axis diagnostics
    Given the axis engine is running and trend detection is active
    When a diagnostic query is issued via IPC
    Then the response SHALL include the current trend state for each monitored axis
