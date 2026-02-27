@REQ-74
Feature: UI settings panel and integration documentation

  @AC-74.1
  Scenario: Settings panel constructs without error
    Given the FlightHub UI settings panel
    When the panel is created via new or default
    Then it SHALL construct without error and its initial state SHALL be valid

  @AC-74.2
  Scenario: Integration docs are loaded and parsed for a simulator
    Given a documentation directory with a simulator markdown file
    When get_simulator_doc is called for that simulator
    Then the document SHALL be parsed with overview files network connections and revert steps populated

  @AC-74.2
  Scenario: Integration docs manager returns error for unknown simulator
    Given an integration docs manager with no docs directory
    When get_simulator_doc is called for a nonexistent simulator
    Then it SHALL return an error containing the simulator name

  @AC-74.3
  Scenario: ValidationResult starts in valid state
    Given a new ValidationResult
    When its validity is checked
    Then it SHALL be valid with no errors or warnings

  @AC-74.3
  Scenario: Adding an error marks result as invalid
    Given a ValidationResult
    When an error is added
    Then is_valid SHALL return false

  @AC-74.3
  Scenario: Adding a warning keeps result valid
    Given a ValidationResult
    When a warning is added
    Then is_valid SHALL still return true

  @AC-74.4
  Scenario: Installer summary deduplicates network ports
    Given two simulator integrations that share a network port
    When both are added to the installer summary
    Then the port SHALL appear only once in network_ports_used

  @AC-74.4
  Scenario: Installer summary accumulates file counts across simulators
    Given two simulators each modifying the same number of files
    When both are added to the installer summary
    Then total_files_modified SHALL equal the combined count
