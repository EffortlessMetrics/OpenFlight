Feature: DCS Radio Frequency
  As a flight simulation enthusiast
  I want dcs radio frequency
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Read active frequency from each radio unit
    Given the system is configured for dcs radio frequency
    When the feature is exercised
    Then dCS adapter reads active radio frequency from each radio unit

  Scenario: Publish frequency changes within one cycle
    Given the system is configured for dcs radio frequency
    When the feature is exercised
    Then frequency changes are published to the event bus within one update cycle

  Scenario: Support VHF and UHF bands
    Given the system is configured for dcs radio frequency
    When the feature is exercised
    Then adapter supports both VHF and UHF frequency bands

  Scenario: Include modulation type with frequency
    Given the system is configured for dcs radio frequency
    When the feature is exercised
    Then radio data includes modulation type (AM/FM) alongside frequency
