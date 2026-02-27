@REQ-424 @product
Feature: Axis Output Rate of Change Limit — Limit Slew Rate

  @AC-424.1
  Scenario: Maximum rate of change per tick is configurable in axis units per tick
    Given an axis configuration with a max_slew_rate field
    When the profile is loaded
    Then the axis pipeline SHALL enforce the specified maximum change per tick

  @AC-424.2
  Scenario: Rate limiter is applied after all other processing stages
    Given an axis with curve, deadzone, blend, and slew-rate limiter configured
    When a tick is processed
    Then the slew-rate limiter SHALL be the last stage applied

  @AC-424.3
  Scenario: Rate limit of 0 disables the limiter
    Given an axis with max_slew_rate set to 0
    When ticks are processed
    Then no rate limiting SHALL be applied (full rate of change allowed)

  @AC-424.4
  Scenario: Property test — output never changes by more than max_slew_rate per tick
    Given any valid input sequence and any positive max_slew_rate value
    When the slew-rate limiter processes consecutive ticks
    Then |output[n] - output[n-1]| SHALL never exceed max_slew_rate

  @AC-424.5
  Scenario: Rate limiter tracks a slew_limited event count per axis
    Given an axis experiencing slew limiting
    When ticks are processed
    Then a per-axis slew_limited_count counter SHALL be incremented for each limited tick

  @AC-424.6
  Scenario: Slew limit events are included in axis diagnostic output
    Given an axis with slew limiting active
    When `flightctl axis diag <axis_id>` is run
    Then the slew_limited_count SHALL be visible in the diagnostic output
