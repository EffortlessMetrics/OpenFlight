@REQ-165 @product
Feature: Multi-engine aircraft profiles

  @AC-165.1
  Scenario: Four-engine throttle axes mapped
    Given a multi-engine profile defining four throttle axes
    When the profile is loaded
    Then throttle 1 SHALL map to axis 0, throttle 2 to axis 1, throttle 3 to axis 2, and throttle 4 to axis 3

  @AC-165.2
  Scenario: Engine start sequence via button mapping
    Given a multi-engine profile is active
    When the engine start sequence button mapping is triggered
    Then the engine start sequence event SHALL be fired for each engine in the defined order

  @AC-165.3
  Scenario: Per-engine mixture control independent
    Given a multi-engine profile with independent mixture controls
    When mixture lever 2 is adjusted
    Then only engine 2 mixture SHALL change while other engines remain unaffected

  @AC-165.4
  Scenario: Prop RPM levers independent
    Given a multi-engine profile with independent prop RPM levers
    When prop lever 3 is adjusted
    Then only engine 3 RPM SHALL change while other engines remain unaffected

  @AC-165.5
  Scenario: Idle detent on all throttle levers
    Given a multi-engine profile with idle detents configured
    When any throttle lever reaches the idle detent position
    Then the idle detent event SHALL be fired for that throttle lever

  @AC-165.6
  Scenario: Full forward detent activates TOGA mode
    Given a multi-engine profile with TOGA detent configured
    When all throttle levers are pushed to the full forward detent
    Then the TOGA mode event SHALL be activated

  @AC-165.7
  Scenario: Throttle reverse range available below idle
    Given a multi-engine profile with reverse thrust configured
    When a throttle lever is moved below the idle detent
    Then the axis output SHALL represent reverse thrust in the defined reverse range

  @AC-165.8
  Scenario: Profile validation requires all four engine axes
    Given a multi-engine profile is being validated
    When any of the four required engine axes is absent from the profile
    Then profile validation SHALL fail with a missing-axis error
