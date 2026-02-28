Feature: Windows MSI Installer
  As a flight simulation enthusiast
  I want windows msi installer
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: MSI installer performs per-user installation without requiring admin privileges
    Given the system is configured for windows msi installer
    When the feature is exercised
    Then mSI installer performs per-user installation without requiring admin privileges

  Scenario: Installer offers optional sim integration checkboxes for MSFS, X-Plane, and DCS
    Given the system is configured for windows msi installer
    When the feature is exercised
    Then installer offers optional sim integration checkboxes for MSFS, X-Plane, and DCS

  Scenario: Installation creates Start Menu shortcut and registers uninstall entry
    Given the system is configured for windows msi installer
    When the feature is exercised
    Then installation creates Start Menu shortcut and registers uninstall entry

  Scenario: Installer validates disk space and prerequisites before proceeding
    Given the system is configured for windows msi installer
    When the feature is exercised
    Then installer validates disk space and prerequisites before proceeding
