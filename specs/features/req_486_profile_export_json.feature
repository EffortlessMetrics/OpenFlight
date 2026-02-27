@REQ-486 @product
Feature: Profile Export to JSON — JSON Format Profile Export  @AC-486.1
  Scenario: flightctl profile export outputs profile as valid JSON
    Given an active profile loaded by the service
    When `flightctl profile export` is executed
    Then the output SHALL be valid JSON parseable by standard JSON tools  @AC-486.2
  Scenario: Exported JSON can be reimported without data loss
    Given a profile exported to JSON
    When the JSON is reimported via `flightctl profile import`
    Then the reimported profile SHALL be semantically identical to the original  @AC-486.3
  Scenario: JSON export includes all axis configs, curves, and metadata
    Given a profile containing axis configurations, curve definitions, and metadata fields
    When the profile is exported to JSON
    Then all axis configs, curves, and metadata SHALL be present and accurate in the export  @AC-486.4
  Scenario: Export format is versioned for future migrations
    Given a profile exported to JSON
    When the JSON structure is inspected
    Then it SHALL contain a format_version field identifying the schema version used
