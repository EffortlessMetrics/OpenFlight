Feature: Profile Merge Preview
  As a flight simulation enthusiast
  I want the CLI to preview merged profiles before applying
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Merged profile shown before applying
    Given multiple profile layers are configured
    When I run the profile preview command
    Then the merged profile output is displayed

  Scenario: Override fields are highlighted
    Given profiles with overlapping settings exist
    When the preview is displayed
    Then overridden fields from each layer are highlighted

  Scenario: User can abort after preview
    Given the merge preview is displayed
    When the user chooses to abort
    Then no profile changes are applied

  Scenario: Preview available in JSON
    Given multiple profile layers are configured
    When I run the profile preview command with --json
    Then the output is valid JSON representing the merged profile
