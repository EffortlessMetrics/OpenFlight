@REQ-372 @product
Feature: Profile Export to JSON — Export Compiled Profile as JSON

  @AC-372.1
  Scenario: flightctl profile export produces valid JSON
    Given a loaded and compiled profile
    When the user runs `flightctl profile export --format json`
    Then the command SHALL produce output that is valid JSON

  @AC-372.2
  Scenario: Exported JSON includes all resolved axes with configured parameters
    Given a compiled profile with multiple configured axes
    When the profile is exported as JSON
    Then the JSON SHALL include all resolved axes with their full parameter sets

  @AC-372.3
  Scenario: JSON schema matches the documented profile JSON schema
    Given a compiled profile exported as JSON
    When the exported JSON is validated against the documented profile JSON schema
    Then validation SHALL pass without errors

  @AC-372.4
  Scenario: Re-importing exported JSON produces identical compiled profile
    Given a profile exported to JSON
    When that JSON is re-imported and compiled
    Then the resulting compiled profile SHALL be identical to the original

  @AC-372.5
  Scenario: Export handles missing optional fields gracefully
    Given a profile with some optional fields not configured
    When the profile is exported as JSON
    Then missing optional fields SHALL be represented as null or omitted without error

  @AC-372.6
  Scenario: Snapshot test — known profile produces known JSON output
    Given a known reference profile
    When the profile is exported as JSON
    Then the output SHALL exactly match the stored snapshot JSON
