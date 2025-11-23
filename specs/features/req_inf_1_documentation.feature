@INF-REQ-1
Feature: Structured Documentation System

  @AC-1.1
  Scenario: Documentation organization
    Given a new documentation file needs to be created
    When the file is placed in the docs directory
    Then it SHALL be organized into one of the bands: requirements, design, concepts, how-to, reference, or adr

  @AC-1.2
  Scenario: Front matter validation
    Given a documentation file is created
    When the file is validated
    Then it SHALL include YAML front matter with doc_id, kind, area, status, and links fields

  @AC-1.3
  Scenario: Stable requirement ID references
    Given documentation that references requirements
    When the documentation is written
    Then it SHALL use stable requirement IDs like REQ-1, INF-REQ-1, or AC-1.1

  @AC-1.4
  Scenario: Unique doc_id validation
    Given multiple documentation files exist
    When the system validates documentation
    Then it SHALL verify all doc_id fields are unique across all files

  @AC-1.5
  Scenario: Documentation index generation
    Given documentation files with front matter exist
    When generating documentation indexes
    Then the system SHALL produce markdown tables grouped by area and kind

  @AC-1.6
  Scenario: Crate documentation coverage
    Given a crate or feature area is referenced in specs or Cargo.toml
    When checking documentation coverage
    Then at least one concept document SHALL exist in docs/concepts/ for that area

  @AC-1.7
  Scenario: Documentation status updates
    Given a documentation file with front matter
    When the documentation status changes
    Then the front matter status field SHALL be updated to reflect the new state
