@REQ-238 @product
Feature: Throttle detents snap axis to fixed positions at configured percentages  @AC-238.1
  Scenario: Detent positions specified as fractions in 0.0 to 1.0 in profile
    Given a throttle axis profile with detent positions configured
    When a detent position value outside [0.0, 1.0] is specified
    Then the profile validation SHALL reject it with a range error  @AC-238.2
  Scenario: Axis within snap_range of detent snaps to exact detent value
    Given a throttle axis with a detent at 0.25 and snap_range of 0.03
    When the physical axis reads a value within 0.03 of 0.25
    Then the output SHALL be exactly 0.25  @AC-238.3
  Scenario: Detent crossing requires moving past snap_range to exit
    Given a throttle axis currently snapped to a detent
    When the physical axis moves but stays within snap_range of that detent
    Then the output SHALL remain snapped to the detent value  @AC-238.4
  Scenario: Multiple detents per axis supported for idle MCT and TOGA
    Given a throttle axis with detents at 0.0 idle 0.75 MCT and 1.0 TOGA positions
    When the physical axis approaches each detent within snap_range
    Then each detent SHALL snap the output independently and correctly  @AC-238.5
  Scenario: Detent snap configurable per axis independently
    Given a profile with two throttle axes each having different detent configurations
    When each axis is evaluated
    Then each axis SHALL apply only its own detent configuration  @AC-238.6
  Scenario: Active detent state emitted on bus for LED indicator logic
    Given a throttle axis with detents configured
    When the axis transitions into or out of a detent
    Then a detent state event with active or inactive status SHALL be emitted on the bus
