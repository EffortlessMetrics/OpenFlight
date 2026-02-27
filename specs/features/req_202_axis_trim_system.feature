@REQ-202 @product
Feature: Axis trim offsets axes independently from pilot input  @AC-202.1
  Scenario: Trim applied as additive offset after pilot input normalization
    Given an axis with pilot input normalized to 0.5
    When a trim offset of 0.1 is active for that axis
    Then the final axis output SHALL be 0.6 reflecting the additive trim  @AC-202.2
  Scenario: Trim range configurable per axis with default of plus or minus 0.3
    Given no explicit trim range is configured for an axis
    When the trim offset is set to 0.35
    Then the trim SHALL be clamped to the default maximum of 0.3  @AC-202.3
  Scenario: Trim increment and decrement via button events at configurable step size
    Given a trim step size of 0.01 is configured for an axis
    When a trim-increment button event is received
    Then the axis trim SHALL increase by exactly 0.01  @AC-202.4
  Scenario: Trim state persists across profile reloads
    Given axis trim values have been set for multiple axes
    When the profile is reloaded
    Then all previously set trim values SHALL be restored to their pre-reload values  @AC-202.5
  Scenario: Trim reset command zeros all axis trims
    Given trim offsets are active on several axes
    When the trim reset command is issued
    Then all axis trim offsets SHALL return to zero  @AC-202.6
  Scenario: Trim visualized in live telemetry dashboard
    Given the live telemetry dashboard is open
    When an axis trim is adjusted
    Then the dashboard SHALL display the updated trim offset for the affected axis
