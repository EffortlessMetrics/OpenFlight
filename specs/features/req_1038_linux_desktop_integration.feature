@REQ-1038
Feature: Linux Desktop Integration
  @AC-1038.1
  Scenario: Application provides a .desktop file for menu integration
    Given the system is configured for REQ-1038
    When the feature condition is met
    Then application provides a .desktop file for menu integration

  @AC-1038.2
  Scenario: Desktop file includes appropriate icon and category metadata
    Given the system is configured for REQ-1038
    When the feature condition is met
    Then desktop file includes appropriate icon and category metadata

  @AC-1038.3
  Scenario: Application icon is installed in standard icon directories
    Given the system is configured for REQ-1038
    When the feature condition is met
    Then application icon is installed in standard icon directories

  @AC-1038.4
  Scenario: Desktop integration follows freedesktop.org specifications
    Given the system is configured for REQ-1038
    When the feature condition is met
    Then desktop integration follows freedesktop.org specifications
