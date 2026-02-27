Feature: Axis Output Rate Limiting
  As a flight simulation enthusiast
  I want axis output updates to be rate-limitable per consumer
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Rate limiting per consumer
    Given an axis consumer has a configured rate limit
    When the axis engine produces output
    Then updates to that consumer are rate-limited

  Scenario: Rate configurable in profile
    Given a consumer rate limit is specified in the profile
    When the profile is loaded
    Then the rate limit is applied

  Scenario: Latest value used
    Given updates are rate-limited
    When the consumer receives an update
    Then it receives the latest value, not a queued older value

  Scenario: RT spine unaffected
    Given rate limiting is active for a consumer
    When the RT spine ticks
    Then the tick rate is unaffected by consumer rate limits
