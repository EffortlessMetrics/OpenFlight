Feature: CLI Plugin Management
  As a flight simulation enthusiast
  I want cli plugin management
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Plugins can be installed, removed, and updated via CLI commands
    Given the system is configured for cli plugin management
    When the feature is exercised
    Then plugins can be installed, removed, and updated via CLI commands

  Scenario: Plugin install verifies signature and compatibility before applying
    Given the system is configured for cli plugin management
    When the feature is exercised
    Then plugin install verifies signature and compatibility before applying

  Scenario: Installed plugins are listed with version and status information
    Given the system is configured for cli plugin management
    When the feature is exercised
    Then installed plugins are listed with version and status information

  Scenario: Plugin update checks for newer versions from configured sources
    Given the system is configured for cli plugin management
    When the feature is exercised
    Then plugin update checks for newer versions from configured sources
