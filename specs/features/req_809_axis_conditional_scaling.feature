Feature: Axis Conditional Scaling
  As a flight simulation enthusiast
  I want axis conditional scaling
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Support scale factors conditioned on sim state
    Given the system is configured for axis conditional scaling
    When the feature is exercised
    Then axis processing supports scale factors conditioned on simulator state variables

  Scenario: Combine conditions with AND/OR operators
    Given the system is configured for axis conditional scaling
    When the feature is exercised
    Then multiple conditions can be combined with logical AND/OR operators

  Scenario: Smooth scale factor transitions
    Given the system is configured for axis conditional scaling
    When the feature is exercised
    Then scale factor transitions are smoothed to avoid abrupt output changes

  Scenario: Validate config at profile load time
    Given the system is configured for axis conditional scaling
    When the feature is exercised
    Then conditional scaling configuration is validated at profile load time
