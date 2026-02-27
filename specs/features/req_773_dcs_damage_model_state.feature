Feature: DCS Damage Model State
  As a flight simulation enthusiast
  I want dcs damage model state
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Per-component damage percentage
    Given the system is configured for dcs damage model state
    When the feature is exercised
    Then dcs adapter exposes per-component damage percentage

  Scenario: Update on aircraft hits
    Given the system is configured for dcs damage model state
    When the feature is exercised
    Then damage state updates when the aircraft takes hits

  Scenario: Publish damage on event bus
    Given the system is configured for dcs damage model state
    When the feature is exercised
    Then damage state is published on the event bus

  Scenario: Total damage summary value
    Given the system is configured for dcs damage model state
    When the feature is exercised
    Then total damage summary is available as a single value
