@REQ-408 @product
Feature: Profile Per-Game Override — Apply Game-Specific Axis Adjustments

  @AC-408.1
  Scenario: Profile supports per-game overrides
    Given a profile with an [overrides.msfs.pitch] section
    When the profile is loaded
    Then the per-game override SHALL be parsed and available

  @AC-408.2
  Scenario: Game-specific values take precedence over base profile values
    Given a base profile value and a conflicting game-specific override
    When the active game is the one specified by the override
    Then the game-specific value SHALL take precedence

  @AC-408.3
  Scenario: Overrides are applied after profile merging before RT compilation
    Given a profile pipeline with merging and RT compilation stages
    When the pipeline executes
    Then game-specific overrides SHALL be applied after merging but before RT compilation

  @AC-408.4
  Scenario: Unknown game names in overrides are logged and ignored
    Given a profile with an override for an unrecognized game name
    When the profile is loaded
    Then a warning SHALL be logged and the unknown override SHALL be ignored

  @AC-408.5
  Scenario: flightctl profile show --game displays the merged profile for that game
    Given a profile with MSFS-specific overrides
    When the user runs `flightctl profile show --game msfs`
    Then the output SHALL show the fully merged profile with MSFS overrides applied

  @AC-408.6
  Scenario: Property test — applying same override twice produces same result
    Given any profile and any game-specific override
    When the override is applied once versus applied twice
    Then both results SHALL be identical (idempotent)
