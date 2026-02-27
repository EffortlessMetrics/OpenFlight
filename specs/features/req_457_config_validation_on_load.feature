@REQ-457 @product
Feature: Config File Validation on Load — Validate All Config Files on Startup

  @AC-457.1
  Scenario: Invalid config files are reported with filename and error location
    Given a service config directory containing a TOML file with a syntax error on line 12
    When the service starts
    Then startup SHALL fail and the error message SHALL include the filename and line number

  @AC-457.2
  Scenario: Unknown config keys are logged as warnings
    Given a valid config file containing an unrecognised key "experimental_turbo_mode"
    When the service loads the config
    Then a warning SHALL be logged identifying the unknown key and its source file

  @AC-457.3
  Scenario: Missing required fields cause startup failure with clear error message
    Given a profile config file missing the required "device_id" field
    When the service attempts to load the profile
    Then startup SHALL be aborted and the error message SHALL name the missing field and file

  @AC-457.4
  Scenario: Config validation summary is included in diagnostic bundle
    Given a service that has started successfully after validating configs
    When a diagnostic bundle is collected
    Then the bundle SHALL include a config validation summary listing all files checked and any warnings
