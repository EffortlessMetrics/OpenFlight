@REQ-70 @REQ-71 @REQ-72 @REQ-73
Feature: Metrics collector trait, dashboard snapshots, registry edge cases, and counter boundary conditions

  @AC-70.1
  Scenario: MetricsRegistry implements MetricsCollector trait collect
    Given a MetricsRegistry with recorded counters
    When collect is called via the MetricsCollector trait
    Then the returned metrics SHALL equal the recorded counters and SHALL match snapshot output

  @AC-70.2
  Scenario: MetricsCollector reset clears all metrics
    Given a MetricsRegistry with counters gauges and histograms recorded
    When reset is called via the MetricsCollector trait
    Then all counters gauges and histograms SHALL be cleared

  @AC-70.3
  Scenario: Custom MetricsCollector implementation works as boxed trait object
    Given a custom struct implementing MetricsCollector
    When it is used as a boxed dyn MetricsCollector
    Then collect and reset methods SHALL be dispatched correctly

  @AC-71.1
  Scenario: MetricsDashboard populates SimMetrics from snapshot
    Given a snapshot with simulator metric counters and histograms
    When MetricsDashboard::from_snapshot is called
    Then all SimMetrics fields SHALL be populated including frames_total and errors_total

  @AC-71.2
  Scenario: MetricsDashboard populates FfbMetrics from snapshot
    Given a snapshot with FFB metric counters and gauges
    When MetricsDashboard::from_snapshot is called
    Then all FfbMetrics fields SHALL be populated including effects_applied_total and emergency_stop_total

  @AC-71.3
  Scenario: MetricsDashboard populates RtMetrics from snapshot
    Given a snapshot with real-time scheduler metrics and a jitter histogram
    When MetricsDashboard::from_snapshot is called
    Then RtMetrics SHALL be populated with correct ticks_total missed_deadlines_total and jitter histogram bounds

  @AC-71.4
  Scenario: Empty snapshot yields default MetricsDashboard values
    Given a snapshot with no metrics recorded
    When MetricsDashboard::from_snapshot is called
    Then all fields SHALL have default values and no panic SHALL occur

  @AC-72.1
  Scenario: Gauge value can be set and read back
    Given a MetricsRegistry
    When a gauge is set to a specific value
    Then gauge_value SHALL return the written value

  @AC-72.2
  Scenario: Counter accumulates increments correctly
    Given a MetricsRegistry
    When a counter is incremented multiple times
    Then the snapshot SHALL reflect the accumulated total

  @AC-72.3
  Scenario: Non-finite histogram observations are silently dropped
    Given a MetricsRegistry
    When NaN or infinite values are observed into a histogram
    Then they SHALL be dropped and only finite samples SHALL appear in the summary

  @AC-72.4
  Scenario: Histogram evicts oldest samples when full
    Given a histogram with a fixed capacity
    When observations exceed the capacity
    Then the oldest samples SHALL be evicted and the count SHALL not exceed capacity

  @AC-72.5
  Scenario: Snapshot contains all metric types
    Given a MetricsRegistry with counters gauges and histogram observations
    When a snapshot is taken
    Then all three metric types SHALL appear in the result

  @AC-72.6
  Scenario: reset is idempotent across multiple calls
    Given a MetricsRegistry
    When reset is called multiple times in succession
    Then each call SHALL succeed without panic and the registry SHALL remain empty

  @AC-73.1
  Scenario: Connection state at threshold 0.5 reports connected
    Given a dashboard snapshot with sim.connection_state set to exactly 0.5
    When the dashboard is built
    Then connected SHALL be true

  @AC-73.1
  Scenario: Connection state below threshold reports disconnected
    Given a dashboard snapshot with sim.connection_state set below 0.5
    When the dashboard is built
    Then connected SHALL be false

  @AC-73.2
  Scenario: Duplicate counter entries use the last value
    Given a raw snapshot slice with two entries for the same counter name
    When MetricsDashboard::from_snapshot is called
    Then the last entry in the slice SHALL take precedence

  @AC-73.3
  Scenario: Large counter values near u64 max are preserved
    Given a counter value set near u64::MAX
    When it is read back through a dashboard snapshot
    Then the exact value SHALL be preserved without truncation or overflow
