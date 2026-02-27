Feature: DCS Radio Frequency Binding
  As a flight simulation enthusiast
  I want the DCS adapter to expose radio frequency data
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Frequency data exposed
    Given DCS is running with radio-equipped aircraft
    When the adapter receives export data
    Then radio frequency data is exposed

  Scenario: Frequency mapped to display
    Given frequency data is available
    When panel display mappings are configured
    Then frequencies are shown on panel displays

  Scenario: Multiple channels supported
    Given the aircraft has multiple radio channels
    When all channels are exported
    Then each channel is available simultaneously

  Scenario: Updates at DCS export rate
    Given DCS is exporting radio data
    When the export cycle completes
    Then frequency data updates at the export rate
