@REQ-27
Feature: Metrics registry counter, gauge, and histogram tracking

  @AC-27.1
  Scenario: Counter and gauge snapshot reflects current values
    Given a metrics registry with a counter and a gauge
    When the counter is incremented and the gauge is set
    Then a snapshot SHALL reflect the updated counter and gauge values

  @AC-27.2
  Scenario: Histogram summary includes percentile data
    Given a metrics registry with a histogram recording several values
    When a summary is requested
    Then the summary SHALL include p50 and p99 percentile estimates

  @AC-27.3
  Scenario: Registry reset clears all recorded metrics
    Given a metrics registry with recorded counters, gauges, and histograms
    When reset is called
    Then all metrics SHALL return to their initial zero state
