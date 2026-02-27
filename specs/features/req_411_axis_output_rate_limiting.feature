@REQ-411 @product
Feature: Axis Output Rate Limiting — Cap Axis Output Change Rate

  @AC-411.1
  Scenario: Rate limiter caps the change in axis value per tick
    Given an axis with a configured maximum slew rate
    When the axis value changes rapidly between ticks
    Then the output change per tick SHALL not exceed the configured maximum slew rate

  @AC-411.2
  Scenario: Maximum slew rate is configurable in units per tick
    Given an axis configuration
    When a max_rate value is specified in units/tick
    Then the rate limiter SHALL enforce that maximum

  @AC-411.3
  Scenario: Rate limiter is applied after curve and deadzone processing
    Given an axis with curve, deadzone, and rate limiter configured
    When a tick is processed
    Then the rate limiter SHALL be applied after curve and deadzone operations

  @AC-411.4
  Scenario: Property test — absolute change between ticks never exceeds max_rate
    Given any valid axis input sequence and any max_rate value
    When the rate limiter is applied over multiple ticks
    Then the absolute output change between consecutive ticks SHALL never exceed max_rate

  @AC-411.5
  Scenario: Rate limiter state resets when axis value is out of valid range
    Given an axis value that falls outside the valid range
    When the rate limiter state is checked
    Then the rate limiter internal state SHALL be reset

  @AC-411.6
  Scenario: Zero rate limit disables the limiter for backward compatibility
    Given an axis with max_rate set to 0
    When ticks are processed
    Then the rate limiter SHALL be effectively disabled (full rate allowed)
