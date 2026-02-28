Feature: Axis Curve Visualization
  As a flight simulation enthusiast
  I want axis curve visualization
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Visual curve editor data provides input-output mapping preview
    Given the system is configured for axis curve visualization
    When the feature is exercised
    Then visual curve editor data provides input-output mapping preview

  Scenario: Curve visualization reflects current profile settings in real time
    Given the system is configured for axis curve visualization
    When the feature is exercised
    Then curve visualization reflects current profile settings in real time

  Scenario: Multiple curve types are rendered including linear, exponential, and custom
    Given the system is configured for axis curve visualization
    When the feature is exercised
    Then multiple curve types are rendered including linear, exponential, and custom

  Scenario: Curve preview shows current input position on the curve graph
    Given the system is configured for axis curve visualization
    When the feature is exercised
    Then curve preview shows current input position on the curve graph