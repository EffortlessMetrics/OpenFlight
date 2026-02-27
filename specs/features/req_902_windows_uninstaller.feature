Feature: Windows Uninstaller
  As a flight simulation enthusiast
  I want windows uninstaller
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Uninstaller removes all binaries and registry entries cleanly
    Given the system is configured for windows uninstaller
    When the feature is exercised
    Then uninstaller removes all binaries and registry entries cleanly

  Scenario: User configuration files in AppData are preserved after uninstall
    Given the system is configured for windows uninstaller
    When the feature is exercised
    Then user configuration files in AppData are preserved after uninstall

  Scenario: Sim-specific plugins installed during setup are removed on uninstall
    Given the system is configured for windows uninstaller
    When the feature is exercised
    Then sim-specific plugins installed during setup are removed on uninstall

  Scenario: Uninstaller provides option to remove all data including user config
    Given the system is configured for windows uninstaller
    When the feature is exercised
    Then uninstaller provides option to remove all data including user config
