@REQ-395 @product
Feature: Axis Output Saturation Protection — Prevent Integrator Windup

  @AC-395.1
  Scenario: Saturation state is entered after 500 ms at max/min output
    Given an axis whose output is at maximum or minimum
    When that condition persists for more than 500 ms
    Then the axis SHALL enter saturation state

  @AC-395.2
  Scenario: Integrating effects are clamped in saturation state
    Given an axis in saturation state
    When trim or PID integral effects are computed
    Then those effects SHALL be clamped to prevent further windup

  @AC-395.3
  Scenario: Saturation state clears when axis returns from extremes
    Given an axis in saturation state
    When the axis output moves away from the extreme boundary
    Then saturation state SHALL be cleared

  @AC-395.4
  Scenario: Saturation events are counted per-axis and exposed via metrics
    Given an axis with saturation protection enabled
    When a saturation event occurs
    Then the per-axis saturation event counter SHALL be incremented and readable via metrics

  @AC-395.5
  Scenario: Saturation threshold and duration are configurable per axis
    Given an axis profile with saturation protection settings
    When the threshold and duration are set
    Then those values SHALL be used for saturation detection for that axis

  @AC-395.6
  Scenario: Property test — saturation protection never causes output to leave [-1, 1]
    Given the saturation protection logic
    When any axis value sequence is processed
    Then the output SHALL always remain within [-1, 1]
