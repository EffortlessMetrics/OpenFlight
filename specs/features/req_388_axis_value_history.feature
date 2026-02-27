@REQ-388 @product
Feature: Axis Value History Ring Buffer  @AC-388.1
  Scenario: Each axis maintains a ring buffer of the last 256 processed values
    Given an axis that has received at least 256 processed values
    When the history buffer is queried
    Then it SHALL contain exactly the last 256 processed axis values in order  @AC-388.2
  Scenario: Ring buffer is fully pre-allocated with no heap allocation on push
    Given the axis history ring buffer initialized at service startup
    When values are pushed to the buffer in the RT loop
    Then no heap allocation SHALL occur during any push operation  @AC-388.3
  Scenario: Buffer is readable without blocking the RT thread
    Given the RT thread is actively writing to the history buffer
    When a non-RT reader accesses the history buffer concurrently
    Then the read SHALL be lock-free and SHALL NOT block the RT thread  @AC-388.4
  Scenario: History is accessible via flightctl axis history
    Given an axis with recent input history recorded
    When the user runs flightctl axis history <axis_id>
    Then the command SHALL display the recent history values for that axis  @AC-388.5
  Scenario: History buffer is separate from the blackbox recording system
    Given both the history ring buffer and the blackbox recorder are active
    When the blackbox recorder is disabled
    Then the per-axis history ring buffer SHALL continue to function independently  @AC-388.6
  Scenario: Property test confirms only the last 256 values remain after 300 pushes
    Given a property test that pushes 300 distinct values to a 256-element ring buffer
    When the buffer contents are read after all pushes
    Then only the last 256 pushed values SHALL be present in insertion order
