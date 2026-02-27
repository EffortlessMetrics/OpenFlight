Feature: DCS Module Auto-Detection
  As a flight simulation enthusiast
  I want the DCS adapter to auto-detect the loaded module type
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Module type is detected
    Given DCS is running with a loaded module
    When the DCS adapter connects
    Then it detects the currently loaded module type

  Scenario: Module mapped to aircraft ID
    Given a module type is detected
    When the adapter processes the module data
    Then the module type is mapped to an aircraft identifier

  Scenario: Detection triggers profile selection
    Given a module is detected and mapped to an aircraft
    When the aircraft identifier is resolved
    Then the corresponding profile is automatically selected

  Scenario: Unknown module uses fallback
    Given DCS is running with an unrecognized module
    When the adapter fails to map the module
    Then it reports the unknown module and uses a fallback profile
