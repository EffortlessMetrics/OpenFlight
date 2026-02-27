@REQ-380 @product
Feature: Combined Input Statistics Across All Active Axes  @AC-380.1
  Scenario: Per-tick statistics include min, max, mean, and std_dev
    Given multiple active axes producing output values each tick
    When per-tick statistics are computed
    Then min, max, mean, and std_dev of all axis outputs SHALL be reported  @AC-380.2
  Scenario: Statistics are computed without heap allocation
    Given the RT thread processing axis statistics each tick
    When statistics are computed across all active axes
    Then no heap allocation SHALL occur during statistics computation  @AC-380.3
  Scenario: Statistics are published to the metrics bus every 1 second
    Given the statistics engine running at 250 Hz
    When 1 second has elapsed
    Then aggregated axis statistics SHALL be published to the metrics bus  @AC-380.4
  Scenario: Statistics are accessible via flightctl stats axes
    Given the service is running with active axes
    When the user runs flightctl stats axes
    Then the command SHALL display current aggregated axis statistics  @AC-380.5
  Scenario: Zeroed axes are excluded from statistics computation
    Given a mix of active and zeroed (not moving) axes
    When statistics are computed
    Then axes with zero output SHALL be excluded from the aggregation  @AC-380.6
  Scenario: Statistics reset to zero on profile change
    Given accumulated axis statistics from the current profile session
    When the active profile changes
    Then all axis statistics SHALL reset to zero immediately after the profile swap
