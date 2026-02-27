Feature: MSFS ATC Interaction
  As a flight simulation enthusiast
  I want the SimConnect adapter to expose ATC interaction data
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: ATC data exposed
    Given MSFS is running with ATC active
    When ATC interaction data is available
    Then the SimConnect adapter exposes it

  Scenario: ATC options mapped to buttons
    Given ATC menu options are available
    When panel button mappings are configured
    Then ATC options are mapped to panel buttons

  Scenario: Selections triggered from hardware
    Given an ATC menu is active
    When the mapped hardware button is pressed
    Then the corresponding ATC response is selected

  Scenario: State changes on event bus
    Given the ATC state changes
    When the adapter processes the change
    Then the new state is published on the event bus
