Feature: Profile Environment Variables
  As a advanced OpenFlight user
  I want profile config to support environment variable substitution
  So that I can share profiles across machines with site-specific values in env vars

  Background:
    Given the OpenFlight service is running

  Scenario: Profile fields can reference $ENV_VAR for runtime values
    Given an environment variable MY_DEVICE is set
    When a profile with a field referencing $MY_DEVICE is loaded
    Then the field value is substituted with the environment variable value

  Scenario: Missing env vars produce a validation warning
    Given an environment variable referenced in a profile is not set
    When the profile is loaded
    Then a validation warning is emitted for the missing variable

  Scenario: Substitution is applied before schema validation
    Given a profile with env var references is loaded
    Then substitution occurs before the schema validator runs

  Scenario: Substituted values are visible in effective config log
    Given env var substitution has been applied
    When the effective config is logged
    Then the log shows the substituted values
