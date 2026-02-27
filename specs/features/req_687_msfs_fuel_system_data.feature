@REQ-687
Feature: MSFS Fuel System Data
  @AC-687.1
  Scenario: Total fuel quantity is published to bus snapshot
    Given the system is configured for REQ-687
    When the feature condition is met
    Then total fuel quantity is published to bus snapshot

  @AC-687.2
  Scenario: Fuel flow per engine is available if sim provides it
    Given the system is configured for REQ-687
    When the feature condition is met
    Then fuel flow per engine is available if sim provides it

  @AC-687.3
  Scenario: Fuel data polling can be toggled in profile
    Given the system is configured for REQ-687
    When the feature condition is met
    Then fuel data polling can be toggled in profile

  @AC-687.4
  Scenario: Fuel channel is documented in sim adapter reference
    Given the system is configured for REQ-687
    When the feature condition is met
    Then fuel channel is documented in sim adapter reference
