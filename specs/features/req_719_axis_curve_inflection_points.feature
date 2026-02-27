Feature: Axis Curve Inflection Points
  As a flight simulation enthusiast
  I want axis curves to support user-defined inflection points
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Inflection points are supported
    Given an axis curve profile is configured
    When user-defined inflection points are specified
    Then the curve applies the inflection points

  Scenario: Inflection points persisted in profile
    Given inflection points are defined for a curve
    When the profile is saved
    Then the inflection points are persisted

  Scenario: Smooth interpolation between points
    Given a curve has multiple inflection points
    When the axis value passes between points
    Then the curve interpolates smoothly

  Scenario: At least 8 points supported
    Given a curve profile is configured
    When 8 inflection points are defined
    Then all 8 points are accepted and applied
