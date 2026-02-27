@REQ-215 @infra
Feature: Service exposes structured metrics for operational monitoring  @AC-215.1
  Scenario: Prometheus-format metrics available at metrics endpoint
    Given the service is running with the metrics endpoint enabled
    When an HTTP GET request is sent to the /metrics endpoint
    Then the response SHALL contain metrics in Prometheus exposition format  @AC-215.2
  Scenario: Tick rate gauge metric present
    Given the service is running
    When the /metrics endpoint is queried
    Then the response SHALL include the openflight_tick_rate_hz gauge with the current tick rate  @AC-215.3
  Scenario: Tick jitter histogram with p50 p95 and p99 present
    Given the service is running and ticks have been processed
    When the /metrics endpoint is queried
    Then the response SHALL include the openflight_tick_jitter_us histogram with p50, p95, and p99 quantiles  @AC-215.4
  Scenario: Device connected counter labeled per VID and PID
    Given one or more HID devices are connected
    When the /metrics endpoint is queried
    Then the response SHALL include openflight_device_connected counters labeled with VID and PID per device  @AC-215.5
  Scenario: Axis clamp total counter present per axis
    Given axes are processing input and clamping has occurred
    When the /metrics endpoint is queried
    Then the response SHALL include openflight_axis_clamp_total counters labeled per axis  @AC-215.6
  Scenario: Metrics reset on service restart and monotonic counters labeled as such
    Given the service has been restarted
    When the /metrics endpoint is queried immediately after restart
    Then all counters SHALL be reset to zero and monotonic counters SHALL carry the monotonic label
