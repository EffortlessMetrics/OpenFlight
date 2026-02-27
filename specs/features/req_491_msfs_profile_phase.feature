@REQ-491 @product
Feature: MSFS Profile Phase Integration — Flight Phase Triggered Profile Transitions  @AC-491.1
  Scenario: Adapter detects parking brake state to trigger parked profile
    Given the MSFS SimConnect adapter is connected
    When the parking brake SimVar transitions to engaged
    Then the adapter SHALL publish a parked phase event on flight-bus  @AC-491.2
  Scenario: Adapter detects gear retraction to trigger climb profile
    Given the MSFS SimConnect adapter is connected
    When the landing gear SimVar transitions to retracted
    Then the adapter SHALL publish a climb phase event on flight-bus  @AC-491.3
  Scenario: Phase events are published on flight-bus within one update cycle
    Given the MSFS SimConnect adapter is connected
    When a phase-triggering SimVar change is received
    Then the corresponding phase event SHALL be published on flight-bus within one update cycle  @AC-491.4
  Scenario: Phase detection thresholds are configurable in profile
    Given a profile with custom phase detection thresholds
    When the profile is loaded by the adapter
    Then the adapter SHALL apply the configured thresholds for phase detection
