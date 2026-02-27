@REQ-1022
Feature: FFB Frequency Response
  @AC-1022.1
  Scenario: FFB effects support tunable frequency characteristics
    Given the system is configured for REQ-1022
    When the feature condition is met
    Then ffb effects support tunable frequency characteristics

  @AC-1022.2
  Scenario: Low-pass and band-pass filtering is available per effect
    Given the system is configured for REQ-1022
    When the feature condition is met
    Then low-pass and band-pass filtering is available per effect

  @AC-1022.3
  Scenario: Frequency response parameters are configurable in profile
    Given the system is configured for REQ-1022
    When the feature condition is met
    Then frequency response parameters are configurable in profile

  @AC-1022.4
  Scenario: Frequency tuning operates within RT processing budget
    Given the system is configured for REQ-1022
    When the feature condition is met
    Then frequency tuning operates within rt processing budget
