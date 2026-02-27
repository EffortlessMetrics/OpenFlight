Feature: DCS Export Script Version Check
  As a flight simulation enthusiast
  I want the DCS export script to check compatibility version
  So that version mismatches are detected and communicated clearly

  Background:
    Given the OpenFlight service is running

  Scenario: Export script sends version in first packet
    When a DCS Export.lua session starts
    Then the export script includes its version number in the first packet sent

  Scenario: Service compares script version against minimum supported
    Given the service receives a DCS export packet with a version field
    When the version is evaluated
    Then the service compares it against the documented minimum supported version

  Scenario: Version mismatch produces user-visible warning
    Given the DCS export script version is below the minimum supported version
    When the service detects the mismatch
    Then a user-visible warning is produced describing the version incompatibility

  Scenario: Updated export script is bundled with installer
    When the OpenFlight installer is inspected
    Then it includes the current compatible DCS Export.lua script
