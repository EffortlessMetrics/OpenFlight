@REQ-189 @product
Feature: Axis values pass through configurable filter chains in order  @AC-189.1
  Scenario: Filters applied in declared pipeline order
    Given an axis is configured with a filter chain of deadzone, expo, and scale stages
    When a raw input value is processed
    Then the value SHALL pass through deadzone then expo then scale before reaching output  @AC-189.2
  Scenario: Removing a filter stage passes value through unchanged
    Given an axis filter chain with the expo stage removed
    When a raw input value is processed
    Then the value SHALL pass through the remaining stages as if the expo stage were an identity transform  @AC-189.3
  Scenario: Filter chain meets 250Hz timing budget
    Given an axis with a fully configured filter chain including deadzone, expo, and scale
    When the filter chain processes 250 ticks per second
    Then each individual axis processing cycle SHALL complete within 1 microsecond  @AC-189.4
  Scenario: Filter chain state persists between ticks
    Given an axis filter chain containing an EMA filter stage
    When consecutive ticks are processed with varying input values
    Then the EMA accumulator state SHALL be correctly carried forward from each tick to the next  @AC-189.5
  Scenario: Profile specifies filter chain per axis by name
    Given a profile with a named filter chain assigned to a specific axis
    When the profile is loaded
    Then the axis SHALL use the named filter chain configuration and no other  @AC-189.6
  Scenario: Invalid filter config rejected at profile load
    Given a profile containing an axis with an unrecognised or malformed filter stage
    When the profile is loaded
    Then a validation error SHALL be returned at load time and the profile SHALL NOT be applied
