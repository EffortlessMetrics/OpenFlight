@REQ-668
Feature: Axis Signal Conditioning
  @AC-668.1
  Scenario: Signal conditioning applies a configurable low-pass filter
    Given the system is configured for REQ-668
    When the feature condition is met
    Then signal conditioning applies a configurable low-pass filter

  @AC-668.2
  Scenario: Filter cutoff frequency is adjustable per axis
    Given the system is configured for REQ-668
    When the feature condition is met
    Then filter cutoff frequency is adjustable per axis

  @AC-668.3
  Scenario: Conditioning preserves signal phase within acceptable tolerance
    Given the system is configured for REQ-668
    When the feature condition is met
    Then conditioning preserves signal phase within acceptable tolerance

  @AC-668.4
  Scenario: Conditioning stage operates within RT budget
    Given the system is configured for REQ-668
    When the feature condition is met
    Then conditioning stage operates within rt budget
