@INF-REQ-17
Feature: Workspace crate metadata validation

  @AC-17.1
  Scenario: Metadata validation report starts in success state
    Given a fresh MetadataValidationReport
    When its success state is checked
    Then it SHALL be successful with no issues

  @AC-17.1
  Scenario: Adding issues marks validation report as failed
    Given a MetadataValidationReport
    When a crate issue is added
    Then is_success SHALL return false

  @AC-17.2
  Scenario: Issue summary lists missing fields
    Given a CrateMetadataIssue with missing version and license
    When the summary is formatted
    Then the output SHALL contain the crate name and list the missing fields

  @AC-17.3
  Scenario: Absolute readme path is returned unchanged
    Given a CratesIoMetadata with an absolute readme path
    When readme_path is called
    Then the absolute path SHALL be returned as-is

  @AC-17.3
  Scenario: Relative readme path is joined to manifest directory
    Given a CratesIoMetadata with a relative readme path
    When readme_path is called with a manifest path
    Then the result SHALL be the readme path joined to the manifest directory

  @AC-17.4
  Scenario: Loading workspace microcrate names includes core crates
    Given the workspace root of the repository
    When load_workspace_microcrate_names is called
    Then flight-core and flight-axis SHALL be present in the result
