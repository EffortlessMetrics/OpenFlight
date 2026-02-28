Feature: Distributed Tracing
  As a flight simulation enthusiast
  I want distributed tracing
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Trace context propagates across IPC boundaries between components
    Given the system is configured for distributed tracing
    When the feature is exercised
    Then trace context propagates across IPC boundaries between components

  Scenario: Traces capture axis processing pipeline stages with timing data
    Given the system is configured for distributed tracing
    When the feature is exercised
    Then traces capture axis processing pipeline stages with timing data

  Scenario: Trace export supports OpenTelemetry protocol for external collectors
    Given the system is configured for distributed tracing
    When the feature is exercised
    Then trace export supports OpenTelemetry protocol for external collectors

  Scenario: Trace sampling rate is configurable to control overhead
    Given the system is configured for distributed tracing
    When the feature is exercised
    Then trace sampling rate is configurable to control overhead
