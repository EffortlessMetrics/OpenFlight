@REQ-1052 @product @user-journey
Feature: Profile lifecycle management
  As a flight sim pilot
  I want to create, edit, delete, import, export, and share profiles
  So that I can manage my controller configurations across devices and simulators

  @AC-1052.1
  Scenario: Create a new profile with custom axis mappings
    Given the OpenFlight service is running with a global profile
    When the user creates a new profile named "My F-16 Setup" for aircraft "F-16C"
    Then the profile SHALL be persisted to the configuration directory
    And the profile SHALL be validated against the profile schema
    And a "profile_created" event SHALL be emitted on the bus with the profile name

  @AC-1052.2
  Scenario: Edit an existing profile and hot-reload changes
    Given a profile named "My F-16 Setup" exists with a deadzone of 5%
    When the user edits the profile to change the deadzone to 3%
    Then the updated profile SHALL be saved and schema-validated
    And the change SHALL be hot-reloaded into the RT spine within one tick boundary
    And the active axis processing SHALL use the new 3% deadzone value

  @AC-1052.3
  Scenario: Delete a profile falls back to global defaults
    Given the active profile is "My F-16 Setup" and a global profile also exists
    When the user deletes the "My F-16 Setup" profile
    Then the profile file SHALL be removed from the configuration directory
    And the service SHALL fall back to the global profile
    And a "profile_deleted" event SHALL be emitted on the bus

  @AC-1052.4
  Scenario: Export and import a profile preserves all settings
    Given a profile named "Custom Helo" exists with axis curves, deadzones, and button mappings
    When the user exports the profile to a JSON file
    And then imports the exported file as a new profile
    Then the imported profile SHALL be identical to the original profile
    And the imported profile SHALL pass schema validation
    And both profiles SHALL produce the same canonical hash

  @AC-1052.5
  Scenario: Share a profile via portable export format
    Given a profile named "Shared Airliner" exists with simulator-specific settings
    When the user exports the profile in shareable format
    Then the export SHALL include profile metadata, version, and device requirements
    And the export SHALL NOT include system-specific paths or credentials
    And another OpenFlight instance SHALL be able to import the shared profile
