@REQ-453 @product
Feature: FFB Condition Effect — Spring, Damper, and Friction Condition Effects

  @AC-453.1
  Scenario: Spring effect creates restoring force proportional to displacement
    Given an FFB device with a spring condition effect configured at coefficient 0.5
    When the stick is displaced to 0.6 from center
    Then the force output SHALL be proportional to 0.6 and directed toward center

  @AC-453.2
  Scenario: Damper effect resists velocity proportional to speed
    Given an FFB device with a damper condition effect configured
    When the stick is moved at a measurable velocity
    Then the force output SHALL oppose the direction of motion and scale with speed

  @AC-453.3
  Scenario: Friction effect provides constant opposing force above threshold
    Given an FFB device with a friction effect and dead-band threshold configured
    When the stick velocity exceeds the dead-band threshold
    Then a constant opposing force SHALL be applied regardless of velocity magnitude

  @AC-453.4
  Scenario: Condition effects can be combined with periodic effects
    Given an FFB device running a spring condition effect
    When a sine periodic effect is added to the same axis
    Then the output force SHALL be the sum of both effects without clipping below the envelope limit
