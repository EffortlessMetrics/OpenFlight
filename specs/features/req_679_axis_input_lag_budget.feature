Feature: Axis Input Lag Budget Tracking
  As a flight simulation enthusiast
  I want the service to track input lag per axis
  So that latency issues are visible and diagnosable

  Background:
    Given the OpenFlight service is running

  Scenario: Per-axis input lag is measured in microseconds
    Given a HID device is connected with at least one axis
    When an axis input sample is received
    Then the lag from HID read to output is recorded in microseconds for that axis

  Scenario: Lag exceeding 5ms triggers a metric increment
    Given an axis is processing input samples
    When the measured input lag for an axis exceeds 5ms
    Then the lag-over-budget metric counter is incremented for that axis

  Scenario: Lag histogram is accessible via metrics endpoint
    Given the service has processed axis input samples
    When the metrics endpoint is queried
    Then the response includes a per-axis input lag histogram

  Scenario: Lag budget is logged in diagnostic bundle
    Given the service has been running with active axes
    When a diagnostic bundle is collected
    Then the bundle contains the axis input lag budget data
