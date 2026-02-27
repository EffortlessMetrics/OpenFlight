@REQ-320 @product
Feature: Sim Racing Force Feedback  @AC-320.1
  Scenario: Service translates racing sim FFB forces to wheel effects
    Given a racing sim telemetry stream is active
    When the sim emits a force feedback command
    Then the service SHALL translate that command into the appropriate wheel FFB effect  @AC-320.2
  Scenario: Force feedback strength is proportional to lateral G-force
    Given the racing sim reports a lateral G-force of 2.5 G
    When the FFB engine processes the telemetry frame
    Then the wheel centering resistance SHALL scale proportionally to the reported lateral G-force  @AC-320.3
  Scenario: Road texture effects are mapped from telemetry vibration data
    Given telemetry data contains road surface vibration amplitudes
    When the FFB engine processes the vibration data
    Then periodic vibration effects SHALL be applied to the wheel matching the telemetry frequency and amplitude  @AC-320.4
  Scenario: Wheel centering spring strength is configurable
    Given a racing sim profile with centering spring strength set to 70%
    When the FFB engine initialises the wheel
    Then the centering spring effect SHALL be applied at 70% of maximum strength  @AC-320.5
  Scenario: FFB effects are disabled when car is parked (speed less than 1 m/s)
    Given the car's current speed is 0.5 m/s
    When the FFB engine evaluates the parked condition
    Then all active FFB effects SHALL be suppressed until speed exceeds 1 m/s  @AC-320.6
  Scenario: Racing sim profiles are separate from flight profiles
    Given both a flight profile and a racing sim profile exist
    When the active profile is switched between them
    Then the flight and racing sim configurations SHALL not interfere with each other
