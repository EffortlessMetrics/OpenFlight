@REQ-522 @product
Feature: Installer Rollback Support

  @AC-522.1 @AC-522.2
  Scenario: flightctl update rollback restores previous version binaries
    Given a successful upgrade from version 1.2.0 to 1.3.0
    When the user runs flightctl update rollback
    Then the service binaries SHALL be restored to version 1.2.0

  @AC-522.3
  Scenario: Rollback preserves user configuration
    Given a user has custom profiles and settings before upgrading
    When a rollback is performed
    Then all user configuration files SHALL remain intact and unchanged

  @AC-522.4
  Scenario: Rollback is only available within 7 days of upgrade
    Given an upgrade was performed more than 7 days ago
    When the user attempts to run flightctl update rollback
    Then the command SHALL return an error indicating rollback window has expired
