@REQ-301 @product
Feature: Axis Deadzone Visualization

  @AC-301.1
  Scenario: CLI displays current deadzone settings for each axis
    Given the service is running with a profile that configures axis deadzones
    When the user runs the CLI deadzone display command
    Then the output SHALL list the deadzone settings for every configured axis

  @AC-301.2
  Scenario: Display shows center deadzone and edge deadzone percentages
    Given an axis with a center deadzone of 5% and an edge deadzone of 3%
    When the CLI deadzone display is invoked for that axis
    Then the output SHALL show both the center deadzone percentage and the edge deadzone percentage

  @AC-301.3
  Scenario: Display shows effective output range after deadzone
    Given an axis configured with center and edge deadzones
    When the CLI deadzone display is invoked
    Then the output SHALL include the effective output range that remains after both deadzones are applied

  @AC-301.4
  Scenario: Visualization updates in real-time when axis is moved
    Given the CLI is displaying live deadzone visualization for an axis
    When the physical axis is moved to a new position
    Then the display SHALL update within 50ms to reflect the new axis position relative to the deadzone regions

  @AC-301.5
  Scenario: Deadzone can be adjusted interactively via CLI
    Given the CLI deadzone interactive editor is active for an axis
    When the user enters a new center deadzone value
    Then the service SHALL accept the new value and apply it to the axis configuration

  @AC-301.6
  Scenario: Changes are reflected immediately in axis processing
    Given the CLI has just applied a new deadzone value to an axis
    When the axis is moved through the deadzone region
    Then the axis output SHALL reflect the updated deadzone without requiring a service restart
