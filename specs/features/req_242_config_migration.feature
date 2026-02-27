@REQ-242 @infra
Feature: Config file schema migrations run automatically on service start  @AC-242.1
  Scenario: Config file includes schema_version field
    Given a valid service configuration file
    When the file is parsed
    Then a schema_version field SHALL be present and contain a non-negative integer  @AC-242.2
  Scenario: Migration applied when schema_version is older than current
    Given a config file with a schema_version older than the current service version
    When the service starts and loads the config
    Then the migration pipeline SHALL apply all pending migrations to bring it to the current version  @AC-242.3
  Scenario: Migrations are idempotent running twice produces same result
    Given a config file that has already been migrated to the current schema_version
    When the migration pipeline is executed again
    Then the resulting config file SHALL be identical to the file before the second run  @AC-242.4
  Scenario: Migration failure preserves original file and logs error
    Given a config file that cannot be migrated due to corrupt data
    When the migration pipeline encounters the error
    Then the original file SHALL remain unchanged and the error SHALL be logged before the service halts  @AC-242.5
  Scenario: Migrated file written atomically via temp file and rename
    Given a config file requiring migration
    When the migration completes
    Then the updated config SHALL be written to a temporary file and then atomically renamed to replace the original  @AC-242.6
  Scenario: Migration history logged in service log at INFO level
    Given a config file that undergoes one or more migrations on service start
    When the service finishes migrating
    Then each migration step applied SHALL be recorded in the service log at INFO level
