Feature: Sim Event Injection
  As a flight simulation enthusiast
  I want sim event injection
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Commands can be sent to simulators through the event injection API
    Given the system is configured for sim event injection
    When the feature is exercised
    Then commands can be sent to simulators through the event injection API

  Scenario: Event injection supports all three simulator platforms with unified interface
    Given the system is configured for sim event injection
    When the feature is exercised
    Then event injection supports all three simulator platforms with unified interface

  Scenario: Injected events are queued and delivered in order with confirmation
    Given the system is configured for sim event injection
    When the feature is exercised
    Then injected events are queued and delivered in order with confirmation

  Scenario: Rate limiting prevents event flooding that could destabilize the simulator
    Given the system is configured for sim event injection
    When the feature is exercised
    Then rate limiting prevents event flooding that could destabilize the simulator