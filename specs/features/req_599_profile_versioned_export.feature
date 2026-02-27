Feature: Profile Versioned Export
  As a flight simulation enthusiast
  I want profile exports to include schema version metadata
  So that profiles can be safely imported across different versions of OpenFlight

  Background:
    Given the OpenFlight service is running with a loaded profile

  Scenario: Exported profiles include OpenFlight version and date
    When a profile is exported
    Then the exported file contains the OpenFlight version and the export date

  Scenario: Profile export format is stable across minor versions
    Given a profile exported from one minor version of OpenFlight
    When it is imported into a newer minor version
    Then the import succeeds without errors

  Scenario: Import validates schema version before loading
    When a profile file is imported
    Then the importer checks the schema version in the file before proceeding

  Scenario: Version mismatch produces a helpful error message
    Given a profile file with an incompatible schema version
    When the profile is imported
    Then the import fails with an error message that identifies the version mismatch and suggests a resolution
