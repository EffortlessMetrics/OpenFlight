@REQ-581 @product
Feature: Axis Engine Core Affinity — Axis engine should support CPU core affinity pinning  @AC-581.1
  Scenario: Axis engine thread is pinnable to a specific CPU core
    Given the axis engine is configured with core affinity set to core 2
    When the axis engine thread starts
    Then the thread SHALL be pinned to CPU core 2  @AC-581.2
  Scenario: Core affinity is configurable in service config
    Given the service configuration file
    When the axis_engine.core_affinity field is set to a valid core number
    Then the service SHALL apply the configured affinity when starting the axis engine  @AC-581.3
  Scenario: Core affinity reduces scheduling jitter
    Given the axis engine is pinned to a dedicated CPU core
    When the axis engine is running at 250Hz
    Then the p99 tick jitter SHALL be within the QG-RT-JITTER budget  @AC-581.4
  Scenario: Core affinity is reported in service diagnostics
    Given the axis engine is running with core affinity set
    When service diagnostics are queried
    Then the diagnostics output SHALL include the configured CPU core affinity value
