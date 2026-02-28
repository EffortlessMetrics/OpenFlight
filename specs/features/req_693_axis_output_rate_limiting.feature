@REQ-693
Feature: Axis Output Rate Limiting
  @AC-693.1
  Scenario: Output update rate can be limited below the processing rate
    Given the system is configured for REQ-693
    When the feature condition is met
    Then output update rate can be limited below the processing rate

  @AC-693.2
  Scenario: Rate limiting reduces sim variable write frequency
    Given the system is configured for REQ-693
    When the feature condition is met
    Then rate limiting reduces sim variable write frequency

  @AC-693.3
  Scenario: Rate limit is configurable per axis in profile
    Given the system is configured for REQ-693
    When the feature condition is met
    Then rate limit is configurable per axis in profile

  @AC-693.4
  Scenario: Rate limiting does not affect internal processing fidelity
    Given the system is configured for REQ-693
    When the feature condition is met
    Then rate limiting does not affect internal processing fidelity
