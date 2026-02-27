Feature: Config File Integrity
  As a flight simulation enthusiast
  I want config file integrity
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Configuration files include HMAC integrity tags validated on load
    Given the system is configured for config file integrity
    When the feature is exercised
    Then configuration files include HMAC integrity tags validated on load

  Scenario: Tampered configuration is detected and reported before applying changes
    Given the system is configured for config file integrity
    When the feature is exercised
    Then tampered configuration is detected and reported before applying changes

  Scenario: Integrity check failure falls back to default configuration with warning
    Given the system is configured for config file integrity
    When the feature is exercised
    Then integrity check failure falls back to default configuration with warning

  Scenario: Integrity validation covers all YAML and TOML configuration files
    Given the system is configured for config file integrity
    When the feature is exercised
    Then integrity validation covers all YAML and TOML configuration files
