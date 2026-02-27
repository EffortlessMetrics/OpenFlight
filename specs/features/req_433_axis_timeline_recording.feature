@REQ-433 @product
Feature: Axis Timeline Recording — Record Value Timeline for Replay and Debug

  @AC-433.1
  Scenario: Timeline records axis value at each pipeline stage with timestamps
    Given timeline recording is enabled for an axis
    When the axis engine processes a tick
    Then a record SHALL be appended for each pipeline stage containing the value and a nanosecond timestamp

  @AC-433.2
  Scenario: Timeline buffer is a fixed-size ring buffer with configurable capacity
    Given a timeline buffer capacity of N is configured
    When more than N records are produced
    Then the oldest records SHALL be overwritten and no heap reallocation SHALL occur

  @AC-433.3
  Scenario: Timeline can be exported as JSON or CSV for analysis
    Given a non-empty timeline buffer
    When an export is requested via CLI or API
    Then the output SHALL be valid JSON or CSV containing all buffered stage records

  @AC-433.4
  Scenario: Recording is enabled and disabled at runtime without pipeline restart
    Given the axis pipeline is running
    When recording is toggled at runtime
    Then recording SHALL start or stop immediately with no pipeline interruption
