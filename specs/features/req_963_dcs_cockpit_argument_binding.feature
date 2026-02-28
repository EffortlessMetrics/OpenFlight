Feature: DCS Cockpit Argument Binding
  As a flight simulation enthusiast
  I want dcs cockpit argument binding
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Cockpit arguments are mapped to device outputs for panel synchronization
    Given the system is configured for dcs cockpit argument binding
    When the feature is exercised
    Then cockpit arguments are mapped to device outputs for panel synchronization

  Scenario: Argument bindings support per-aircraft module configurations
    Given the system is configured for dcs cockpit argument binding
    When the feature is exercised
    Then argument bindings support per-aircraft module configurations

  Scenario: Binding updates are delivered within one processing tick of argument change
    Given the system is configured for dcs cockpit argument binding
    When the feature is exercised
    Then binding updates are delivered within one processing tick of argument change

  Scenario: Invalid cockpit argument references are reported as configuration warnings
    Given the system is configured for dcs cockpit argument binding
    When the feature is exercised
    Then invalid cockpit argument references are reported as configuration warnings