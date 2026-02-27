@REQ-185 @product
Feature: Axis scaling and response curve configuration operates correctly  @AC-185.1
  Scenario: Linear scale factor applied uniformly
    Given an axis with a linear scale factor configured in the profile
    When axis input is received across the full range
    Then the scale factor SHALL be applied uniformly to the axis output  @AC-185.2
  Scenario: S-curve reduces sensitivity near centre
    Given an axis with a cubic expo (S-curve) response configured
    When axis input near centre is processed
    Then the output sensitivity SHALL be reduced near centre relative to a linear response  @AC-185.3
  Scenario: Custom lookup-table curve interpolated smoothly
    Given an axis with a custom lookup-table curve defined by control points
    When axis input falls between defined control points
    Then the output SHALL be smoothly interpolated between the surrounding points  @AC-185.4
  Scenario: Multiple curve stages can be chained
    Given an axis profile with deadzone, expo, and scale stages chained in sequence
    When raw axis input is processed
    Then all stages SHALL be applied in order: deadzone then expo then scale  @AC-185.5
  Scenario: Curve configuration survives hot-swap profile reload
    Given an axis with a configured response curve that is active during simulation
    When a new profile is hot-swapped without restarting the service
    Then the new curve configuration SHALL be active for subsequent axis inputs  @AC-185.6
  Scenario: Curve output always bounded to [-1.0, 1.0]
    Given an axis with any curve configuration applied
    When extreme or out-of-range inputs are processed
    Then the curve output SHALL always be bounded to the range [-1.0, 1.0]
