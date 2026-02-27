Feature: Profile Dry Run
  As a flight simulation enthusiast
  I want to preview profile changes before applying them
  So that I can validate my configuration without disrupting my session

  Background:
    Given the OpenFlight service is running
    And a profile is currently active

  Scenario: Dry run shows diff without modifying active profile
    Given a modified profile YAML is ready to apply
    When I run "flightctl profile apply myprofile.yaml --dry-run"
    Then the output shows what axes and rules would change
    And the currently active profile remains unchanged

  Scenario: Dry run reports schema validation errors
    Given a profile YAML with an invalid axis range value
    When I run "flightctl profile apply invalid.yaml --dry-run"
    Then the output contains schema validation error details
    And the exit code is non-zero

  Scenario: Dry run compares against current active profile
    Given the active profile has axis "PITCH" with deadzone 0.05
    And the candidate profile changes the deadzone to 0.10
    When I run "flightctl profile apply candidate.yaml --dry-run"
    Then the diff output highlights the deadzone change for axis "PITCH"
