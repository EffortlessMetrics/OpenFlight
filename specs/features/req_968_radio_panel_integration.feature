Feature: Radio Panel Integration
  As a flight simulation enthusiast
  I want radio panel integration
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Radio frequency management synchronizes physical panel with sim radios
    Given the system is configured for radio panel integration
    When the feature is exercised
    Then radio frequency management synchronizes physical panel with sim radios

  Scenario: Active and standby frequencies are displayed on panel LCD or LEDs
    Given the system is configured for radio panel integration
    When the feature is exercised
    Then active and standby frequencies are displayed on panel LCD or LEDs

  Scenario: Frequency swap operation on panel triggers corresponding sim event
    Given the system is configured for radio panel integration
    When the feature is exercised
    Then frequency swap operation on panel triggers corresponding sim event

  Scenario: Radio panel integration supports COM, NAV, and ADF frequency bands
    Given the system is configured for radio panel integration
    When the feature is exercised
    Then radio panel integration supports COM, NAV, and ADF frequency bands