@REQ-1009
Feature: Template Library
  @AC-1009.1
  Scenario: Mapping templates are available for common aircraft and device combinations
    Given the system is configured for REQ-1009
    When the feature condition is met
    Then mapping templates are available for common aircraft and device combinations

  @AC-1009.2
  Scenario: Templates can be exported as shareable files
    Given the system is configured for REQ-1009
    When the feature condition is met
    Then templates can be exported as shareable files

  @AC-1009.3
  Scenario: Imported templates are validated against schema before application
    Given the system is configured for REQ-1009
    When the feature condition is met
    Then imported templates are validated against schema before application

  @AC-1009.4
  Scenario: Template library is searchable by aircraft type and device model
    Given the system is configured for REQ-1009
    When the feature condition is met
    Then template library is searchable by aircraft type and device model
