@REQ-410 @product
Feature: Batch Axis Configuration — Configure Multiple Axes with a Single Command

  @AC-410.1
  Scenario: flightctl axis set-all --deadzone applies to all active axes
    Given multiple active axes
    When the user runs `flightctl axis set-all --deadzone 0.05`
    Then all active axes SHALL have their deadzone updated to 0.05

  @AC-410.2
  Scenario: Batch command supports deadzone, expo, invert, and scale parameters
    Given the `flightctl axis set-all` command
    When called with --deadzone, --expo, --invert, or --scale flags
    Then each parameter SHALL be applied to all active axes

  @AC-410.3
  Scenario: Batch operation is atomic — all axes update or none
    Given multiple active axes and a batch configuration command
    When the batch update is applied
    Then either all axes SHALL be updated or none SHALL be updated (atomic)

  @AC-410.4
  Scenario: Individual per-axis overrides survive batch updates
    Given an axis with a manually set per-axis override
    When a batch update is applied that would affect that axis
    Then the individual per-axis override SHALL be preserved

  @AC-410.5
  Scenario: Batch changes are persisted to the profile file
    Given a batch axis configuration command that completes successfully
    When the profile file is inspected
    Then the batch changes SHALL be persisted

  @AC-410.6
  Scenario: Dry-run mode shows what would change without applying
    Given a batch command with the --dry-run flag
    When the command is executed
    Then it SHALL display what would be changed without applying any changes
