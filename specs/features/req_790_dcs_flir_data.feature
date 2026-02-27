Feature: DCS FLIR Data
  As a flight simulation enthusiast
  I want dcs flir data
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Expose FLIR and targeting pod status
    Given the system is configured for dcs flir data
    When the feature is exercised
    Then dcs adapter exposes flir and targeting pod status when available

  Scenario: Include lock state and coordinates
    Given the system is configured for dcs flir data
    When the feature is exercised
    Then flir data includes lock state and target coordinates

  Scenario: Publish on event bus
    Given the system is configured for dcs flir data
    When the feature is exercised
    Then data is published on the event bus

  Scenario: Well-defined absent status
    Given the system is configured for dcs flir data
    When the feature is exercised
    Then unavailable flir data returns a well-defined absent status
