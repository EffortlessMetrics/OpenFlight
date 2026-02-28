@REQ-1026
Feature: FFB Gear Buffet
  @AC-1026.1
  Scenario: Landing gear deployment produces configurable vibration effect
    Given the system is configured for REQ-1026
    When the feature condition is met
    Then landing gear deployment produces configurable vibration effect

  @AC-1026.2
  Scenario: Buffet duration matches gear transit time from sim data
    Given the system is configured for REQ-1026
    When the feature condition is met
    Then buffet duration matches gear transit time from sim data

  @AC-1026.3
  Scenario: Gear buffet intensity is configurable per aircraft profile
    Given the system is configured for REQ-1026
    When the feature condition is met
    Then gear buffet intensity is configurable per aircraft profile

  @AC-1026.4
  Scenario: Effect is suppressed when gear is fully deployed or retracted
    Given the system is configured for REQ-1026
    When the feature condition is met
    Then effect is suppressed when gear is fully deployed or retracted
