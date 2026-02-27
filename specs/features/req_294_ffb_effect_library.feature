@REQ-294 @product
Feature: FFB Effect Library  @AC-294.1
  Scenario: Service exposes a built-in library of named FFB effects
    Given a connected FFB-capable device
    When the service is queried for available named effects
    Then it SHALL return a list including at least the built-in named effects  @AC-294.2
  Scenario: Built-in effects include spring centering, friction, rumble, and constant force
    Given the built-in FFB effect library
    When the available effect names are inspected
    Then the library SHALL contain effects named "spring_centering", "friction", "rumble", and "constant_force"  @AC-294.3
  Scenario: Effects can be triggered by profile actions
    Given a profile action configured to trigger the "rumble" effect
    When the action is activated via a button press
    Then the "rumble" effect SHALL play on the connected FFB device  @AC-294.4
  Scenario: Effect strength is configurable from 0 to 100 percent
    Given a profile action triggering "spring_centering" with strength set to 50
    When the action fires
    Then the spring centering effect SHALL play at 50% of its maximum force  @AC-294.5
  Scenario: Multiple effects can be blended simultaneously up to four
    Given a profile that activates "spring_centering", "friction", "rumble", and "constant_force" at the same time
    When all four actions fire concurrently
    Then all four effects SHALL play simultaneously and the total output SHALL be their blended sum  @AC-294.6
  Scenario: FFB library operates independently of simulator telemetry
    Given the service is running with no simulator connected
    When a profile action triggers a named FFB effect
    Then the effect SHALL play on the FFB device without requiring any simulator telemetry data
