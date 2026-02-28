Feature: Silent Install
  As a flight simulation enthusiast
  I want silent install
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Installer supports silent mode via command-line flag for unattended operation
    Given the system is configured for silent install
    When the feature is exercised
    Then installer supports silent mode via command-line flag for unattended operation

  Scenario: Silent install accepts configuration parameters for sim integration selection
    Given the system is configured for silent install
    When the feature is exercised
    Then silent install accepts configuration parameters for sim integration selection

  Scenario: Silent install returns non-zero exit code on failure with error in log
    Given the system is configured for silent install
    When the feature is exercised
    Then silent install returns non-zero exit code on failure with error in log

  Scenario: Silent install produces machine-readable log for enterprise deployment tools
    Given the system is configured for silent install
    When the feature is exercised
    Then silent install produces machine-readable log for enterprise deployment tools
