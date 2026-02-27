@REQ-479 @product
Feature: FFB Ramp Effect — Linearly Increasing Force  @AC-479.1
  Scenario: Ramp effect increases force linearly over configured duration
    Given an FFB ramp effect configured with a 1-second duration
    When the effect is started
    Then the output force SHALL increase linearly from start force to end force over 1 second  @AC-479.2
  Scenario: Ramp can be configured with start force, end force, and duration
    Given an FFB ramp effect definition
    When the effect is created with start_force=0, end_force=10000, and duration=2s
    Then the effect SHALL accept and apply those parameters without error  @AC-479.3
  Scenario: Ramp loops or stops at completion according to configuration
    Given two ramp effects, one configured to loop and one configured to stop
    When both effects complete their configured duration
    Then the looping ramp SHALL restart from start force and the stopping ramp SHALL output zero force  @AC-479.4
  Scenario: Ramp is composable with other effects
    Given an active ramp effect and an active spring effect on the same axis
    When both effects are active simultaneously
    Then the FFB engine SHALL combine their outputs correctly within the safety envelope
