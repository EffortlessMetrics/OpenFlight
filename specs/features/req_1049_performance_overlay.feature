@REQ-1049
Feature: Performance Overlay
  @AC-1049.1
  Scenario: In-game overlay displays processing latency and jitter metrics
    Given the system is configured for REQ-1049
    When the feature condition is met
    Then in-game overlay displays processing latency and jitter metrics

  @AC-1049.2
  Scenario: Overlay shows per-axis update rates and queue depths
    Given the system is configured for REQ-1049
    When the feature condition is met
    Then overlay shows per-axis update rates and queue depths

  @AC-1049.3
  Scenario: Performance warnings are highlighted when thresholds are exceeded
    Given the system is configured for REQ-1049
    When the feature condition is met
    Then performance warnings are highlighted when thresholds are exceeded

  @AC-1049.4
  Scenario: Overlay can be toggled independently of input overlay
    Given the system is configured for REQ-1049
    When the feature condition is met
    Then overlay can be toggled independently of input overlay
