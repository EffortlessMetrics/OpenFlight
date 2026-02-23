@INF-REQ-13
Feature: Test helper utilities for integration and unit testing

  @AC-13.1
  Scenario: Config builder provides sensible defaults
    Given a test config builder with no explicit parameters
    When the config is built
    Then the resulting config SHALL have valid default values for all fields

  @AC-13.1
  Scenario: Device builder customization applies overrides
    Given a test device builder with custom name and type
    When the device is built
    Then the resulting device SHALL reflect all customized fields

  @AC-13.2
  Scenario: Test harness enforces timeouts
    Given a test harness configured with a short timeout
    When a test operation exceeds the timeout
    Then the harness SHALL report a timeout failure

  @AC-13.3
  Scenario: Temporary directory is created and cleaned up
    Given a request to create a temporary test directory
    When the temp dir is created
    Then it SHALL exist on disk and be usable for file operations

  @AC-13.3
  Scenario: Wait-for-condition succeeds when condition is met
    Given a condition that becomes true after a short delay
    When wait_for_condition is called with a sufficient timeout
    Then it SHALL return success once the condition is met
