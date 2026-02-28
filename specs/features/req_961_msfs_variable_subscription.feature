Feature: MSFS Variable Subscription
  As a flight simulation enthusiast
  I want msfs variable subscription
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Efficient SimVar polling subscribes only to variables required by active profile
    Given the system is configured for msfs variable subscription
    When the feature is exercised
    Then efficient SimVar polling subscribes only to variables required by active profile

  Scenario: Variable subscription updates when profile or aircraft changes
    Given the system is configured for msfs variable subscription
    When the feature is exercised
    Then variable subscription updates when profile or aircraft changes

  Scenario: Subscription batching reduces SimConnect round trips for performance
    Given the system is configured for msfs variable subscription
    When the feature is exercised
    Then subscription batching reduces SimConnect round trips for performance

  Scenario: Unsubscribed variables are automatically cleaned up on profile switch
    Given the system is configured for msfs variable subscription
    When the feature is exercised
    Then unsubscribed variables are automatically cleaned up on profile switch