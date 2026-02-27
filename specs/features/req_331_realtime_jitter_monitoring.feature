@REQ-331 @product
Feature: Real-Time Jitter Monitoring  @AC-331.1
  Scenario: Service measures tick timing jitter at 250Hz
    Given the service is running at 250Hz
    When ticks are processed
    Then the service SHALL record the deviation of each tick interval from the nominal 4ms period  @AC-331.2
  Scenario: p99 jitter metric is exposed via Prometheus
    Given the Prometheus metrics endpoint is enabled
    When jitter measurements have been collected
    Then the endpoint SHALL expose a rt_jitter_p99_ms gauge reflecting the p99 jitter value  @AC-331.3
  Scenario: Warning is logged when p99 jitter exceeds 0.5ms
    Given the service is monitoring jitter
    When the computed p99 jitter exceeds 0.5ms
    Then the service SHALL emit a WARN log entry indicating the threshold breach  @AC-331.4
  Scenario: Jitter history is kept in a ring buffer of 1000 ticks
    Given the jitter ring buffer has capacity 1000
    When more than 1000 ticks have elapsed
    Then the oldest entries SHALL be overwritten and only the last 1000 measurements are retained  @AC-331.5
  Scenario: CLI displays jitter histogram via flightctl jitter
    Given the service is running and jitter data is available
    When the user runs flightctl jitter
    Then the CLI SHALL display a histogram of recent jitter measurements  @AC-331.6
  Scenario: Jitter monitoring has negligible per-measurement overhead
    Given jitter measurement is active
    When the overhead of a single jitter measurement is benchmarked
    Then the overhead SHALL be less than 1µs per measurement
