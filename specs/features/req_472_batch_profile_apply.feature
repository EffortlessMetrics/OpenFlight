@REQ-472 @product
Feature: Batch Profile Apply — Atomic Multi-Change Profile Patching  @AC-472.1
  Scenario: flightctl profile apply accepts a JSON patch document
    Given a running service and a valid JSON patch document
    When `flightctl profile apply --patch patch.json` is executed
    Then all changes in the patch SHALL be submitted to the service for application  @AC-472.2
  Scenario: Batch apply is atomic — all or nothing
    Given a patch document with three changes where the third is invalid
    When `flightctl profile apply` is executed
    Then none of the three changes SHALL be applied and an error SHALL be reported  @AC-472.3
  Scenario: Dry-run mode shows changes without applying them
    Given a valid patch document
    When `flightctl profile apply --dry-run` is executed
    Then the command SHALL print a diff of what would change and exit without modifying the profile  @AC-472.4
  Scenario: Applied changes are reported as a diff summary
    Given a valid patch document is applied successfully
    When the apply command completes
    Then the command output SHALL include a human-readable diff summary of the applied changes
