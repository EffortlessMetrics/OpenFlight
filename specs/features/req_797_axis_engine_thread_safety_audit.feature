Feature: Axis Engine Thread Safety Audit
  As a flight simulation enthusiast
  I want axis engine thread safety audit
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Pass thread safety audit
    Given the system is configured for axis engine thread safety audit
    When the feature is exercised
    Then axis engine code passes a documented thread safety audit

  Scenario: Atomic or lock-free shared state
    Given the system is configured for axis engine thread safety audit
    When the feature is exercised
    Then all shared mutable state uses atomic or lock-free primitives

  Scenario: No data races via type system
    Given the system is configured for axis engine thread safety audit
    When the feature is exercised
    Then no data races are possible according to the rust type system

  Scenario: Document audit findings
    Given the system is configured for axis engine thread safety audit
    When the feature is exercised
    Then audit findings are documented and tracked
