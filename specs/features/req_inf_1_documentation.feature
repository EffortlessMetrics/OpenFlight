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
