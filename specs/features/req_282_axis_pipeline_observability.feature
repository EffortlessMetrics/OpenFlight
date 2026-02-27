@REQ-282 @product
Feature: Axis pipeline observability with per-stage latency counters and Prometheus metrics endpoint  @AC-282.1
  Scenario: Each pipeline stage emits per-axis latency counter
    Given the axis processing pipeline is running at 250Hz
    When a processing tick completes
    Then each pipeline stage SHALL record the elapsed nanoseconds for every active axis  @AC-282.2
  Scenario: Stage counters are accessible via Prometheus metrics endpoint
    Given the metrics endpoint is enabled and the service is running
    When the Prometheus scrape endpoint is queried
    Then the response SHALL include axis pipeline stage counter metrics in Prometheus text format  @AC-282.3
  Scenario: Counter names follow flight_axis_stage stage_name ns format
    Given the Prometheus metrics endpoint returns axis stage counters
    When the metric names are inspected
    Then each counter name SHALL match the pattern flight_axis_stage_{stage_name}_ns  @AC-282.4
  Scenario: Metrics are updated every tick without allocation
    Given the axis pipeline is processing ticks on the RT spine
    When the allocator instrumentation is active during a metrics update tick
    Then the metrics update path SHALL complete with zero heap allocations  @AC-282.5
  Scenario: Metrics endpoint is served on configurable port with default 9090
    Given the service configuration does not specify a metrics port
    When the service starts and the metrics endpoint is probed
    Then the endpoint SHALL be reachable on port 9090 by default and on the configured port when overridden  @AC-282.6
  Scenario: CLI can display pipeline stage timing summary
    Given the service is running with metrics enabled
    When the user runs the CLI diagnostics timing subcommand
    Then a table of pipeline stage names and their recent latency percentiles SHALL be printed to stdout
