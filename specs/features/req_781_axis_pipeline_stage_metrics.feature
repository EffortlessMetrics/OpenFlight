Feature: Axis Pipeline Stage Metrics
  As a flight simulation enthusiast
  I want axis pipeline stage metrics
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Report per-stage processing time
    Given the system is configured for axis pipeline stage metrics
    When the feature is exercised
    Then each axis pipeline stage reports its processing time

  Scenario: No allocation in metric collection
    Given the system is configured for axis pipeline stage metrics
    When the feature is exercised
    Then stage metrics are collected without allocating on the rt path

  Scenario: Configurable aggregation windows
    Given the system is configured for axis pipeline stage metrics
    When the feature is exercised
    Then metrics are aggregated over configurable windows

  Scenario: Queryable via gRPC
    Given the system is configured for axis pipeline stage metrics
    When the feature is exercised
    Then stage metrics are queryable via grpc api
