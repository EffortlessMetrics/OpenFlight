@REQ-250 @infra
Feature: COMPATIBILITY.md is automatically generated and kept current in CI  @AC-250.1
  Scenario: cargo xtask gen-compat generates COMPATIBILITY.md from YAML manifests
    Given the device YAML manifests are present in the repository
    When cargo xtask gen-compat is executed
    Then COMPATIBILITY.md SHALL be regenerated from the manifests and written to the repository root  @AC-250.2
  Scenario: CI fails if COMPATIBILITY.md is outdated
    Given COMPATIBILITY.md was generated from an older set of manifests
    When the CI stale-check step runs
    Then the CI job SHALL fail and report that COMPATIBILITY.md requires regeneration  @AC-250.3
  Scenario: Manifest schema validated in CI for required fields and valid tier values
    Given a device YAML manifest under review in a pull request
    When the CI manifest-validation step runs
    Then the step SHALL verify all required fields are present and all tier values are valid  @AC-250.4
  Scenario: JSON export of compatibility data published as CI artifact
    Given a passing CI run with gen-compat enabled
    When the CI pipeline completes
    Then a JSON export of the compatibility data SHALL be published as a named CI artifact  @AC-250.5
  Scenario: Tier statistics included in generated COMPATIBILITY.md output
    Given device manifests with tier T1, T2, and T3 classifications
    When COMPATIBILITY.md is generated
    Then the document SHALL include a summary section with the count of devices at each tier  @AC-250.6
  Scenario: New manifest without support.tier field fails CI validation
    Given a new device YAML manifest that omits the support.tier field
    When the CI manifest-validation step runs
    Then the validation SHALL fail with an error identifying the missing support.tier field
