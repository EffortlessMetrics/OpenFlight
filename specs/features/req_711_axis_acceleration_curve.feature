@REQ-711
Feature: Axis Acceleration Curve
  @AC-711.1
  Scenario: Acceleration curve scales sensitivity based on input velocity
    Given the system is configured for REQ-711
    When the feature condition is met
    Then acceleration curve scales sensitivity based on input velocity

  @AC-711.2
  Scenario: Faster movements produce proportionally larger output changes
    Given the system is configured for REQ-711
    When the feature condition is met
    Then faster movements produce proportionally larger output changes

  @AC-711.3
  Scenario: Acceleration factor is configurable from 1x to 10x
    Given the system is configured for REQ-711
    When the feature condition is met
    Then acceleration factor is configurable from 1x to 10x

  @AC-711.4
  Scenario: Acceleration applies only in relative mode
    Given the system is configured for REQ-711
    When the feature condition is met
    Then acceleration applies only in relative mode
