@REQ-1024
Feature: FFB Airspeed Scaling
  @AC-1024.1
  Scenario: FFB effect magnitude scales with simulated airspeed
    Given the system is configured for REQ-1024
    When the feature condition is met
    Then ffb effect magnitude scales with simulated airspeed

  @AC-1024.2
  Scenario: Scaling curve is configurable per aircraft profile
    Given the system is configured for REQ-1024
    When the feature condition is met
    Then scaling curve is configurable per aircraft profile

  @AC-1024.3
  Scenario: WHEN airspeed data is unavailable THEN effects use safe default scaling
    Given the system is configured for REQ-1024
    When the feature condition is met
    Then when airspeed data is unavailable then effects use safe default scaling

  @AC-1024.4
  Scenario: Scaling factor is clamped to prevent excessive force output
    Given the system is configured for REQ-1024
    When the feature condition is met
    Then scaling factor is clamped to prevent excessive force output
