Feature: Axis Processing Profiler
  As a flight simulation enthusiast
  I want the axis engine to have a processing time profiler
  So that I can identify performance bottlenecks in the processing pipeline

  Background:
    Given the OpenFlight service is running with axis processing active

  Scenario: Per-stage processing time is measured in nanoseconds
    Given the axis processing profiler is enabled
    When the axis engine completes a processing tick
    Then the elapsed time for each pipeline stage is recorded in nanoseconds

  Scenario: Profile results are accessible via gRPC diagnostics RPC
    Given the axis processing profiler has collected data
    When the gRPC diagnostics RPC is called for axis profiling
    Then the response contains per-stage timing data

  Scenario: Profiler can be enabled and disabled at runtime
    Given the axis processing profiler is disabled
    When the profiler is enabled via the gRPC control interface
    Then profiling data begins to be collected without restarting the service
    When the profiler is disabled again
    Then profiling data collection stops

  Scenario: Profiling overhead is less than 1 microsecond per stage
    Given the axis processing profiler is enabled
    When profiling overhead is measured
    Then the overhead per pipeline stage is less than 1 microsecond
