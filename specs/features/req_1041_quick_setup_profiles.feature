@REQ-1041
Feature: Quick Setup Profiles
  @AC-1041.1
  Scenario: One-click setup configures common aircraft and device combinations
    Given the system is configured for REQ-1041
    When the feature condition is met
    Then one-click setup configures common aircraft and device combinations

  @AC-1041.2
  Scenario: Quick setup detects connected devices and suggests matching profiles
    Given the system is configured for REQ-1041
    When the feature condition is met
    Then quick setup detects connected devices and suggests matching profiles

  @AC-1041.3
  Scenario: Setup wizard completes in fewer than 5 user interactions
    Given the system is configured for REQ-1041
    When the feature condition is met
    Then setup wizard completes in fewer than 5 user interactions

  @AC-1041.4
  Scenario: Quick setup results can be customized after initial configuration
    Given the system is configured for REQ-1041
    When the feature condition is met
    Then quick setup results can be customized after initial configuration
