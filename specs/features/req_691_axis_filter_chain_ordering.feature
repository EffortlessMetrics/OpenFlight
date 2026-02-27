@REQ-691
Feature: Axis Filter Chain Ordering
  @AC-691.1
  Scenario: Filter chain order is defined in profile configuration
    Given the system is configured for REQ-691
    When the feature condition is met
    Then filter chain order is defined in profile configuration

  @AC-691.2
  Scenario: Reordering filters produces deterministic output changes
    Given the system is configured for REQ-691
    When the feature condition is met
    Then reordering filters produces deterministic output changes

  @AC-691.3
  Scenario: Invalid filter chain configurations are rejected at load time
    Given the system is configured for REQ-691
    When the feature condition is met
    Then invalid filter chain configurations are rejected at load time

  @AC-691.4
  Scenario: Default filter chain order matches recommended signal path
    Given the system is configured for REQ-691
    When the feature condition is met
    Then default filter chain order matches recommended signal path
