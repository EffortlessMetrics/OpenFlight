@REQ-701
Feature: Axis Exponential Response
  @AC-701.1
  Scenario: Exponential curve provides fine control near center
    Given the system is configured for REQ-701
    When the feature condition is met
    Then exponential curve provides fine control near center

  @AC-701.2
  Scenario: Exponent value is configurable from 1.0 to 5.0
    Given the system is configured for REQ-701
    When the feature condition is met
    Then exponent value is configurable from 1.0 to 5.0

  @AC-701.3
  Scenario: Exponential response preserves endpoint mapping
    Given the system is configured for REQ-701
    When the feature condition is met
    Then exponential response preserves endpoint mapping

  @AC-701.4
  Scenario: Negative exponents are rejected with descriptive error
    Given the system is configured for REQ-701
    When the feature condition is met
    Then negative exponents are rejected with descriptive error
