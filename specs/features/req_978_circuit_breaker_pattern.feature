Feature: Circuit Breaker Pattern
  As a flight simulation enthusiast
  I want circuit breaker pattern
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Circuit breakers prevent cascading failures between system components
    Given the system is configured for circuit breaker pattern
    When the feature is exercised
    Then circuit breakers prevent cascading failures between system components

  Scenario: Circuit state transitions through closed, open, and half-open states
    Given the system is configured for circuit breaker pattern
    When the feature is exercised
    Then circuit state transitions through closed, open, and half-open states

  Scenario: Failure threshold and recovery timeout are configurable per circuit
    Given the system is configured for circuit breaker pattern
    When the feature is exercised
    Then failure threshold and recovery timeout are configurable per circuit

  Scenario: Circuit breaker state changes are emitted as events on the system bus
    Given the system is configured for circuit breaker pattern
    When the feature is exercised
    Then circuit breaker state changes are emitted as events on the system bus