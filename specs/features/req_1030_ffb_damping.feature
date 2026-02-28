@REQ-1030
Feature: FFB Damping
  @AC-1030.1
  Scenario: Velocity-dependent damping resists rapid stick movements
    Given the system is configured for REQ-1030
    When the feature condition is met
    Then velocity-dependent damping resists rapid stick movements

  @AC-1030.2
  Scenario: Damping coefficient is configurable per axis in profile
    Given the system is configured for REQ-1030
    When the feature condition is met
    Then damping coefficient is configurable per axis in profile

  @AC-1030.3
  Scenario: Damping force scales linearly with movement velocity
    Given the system is configured for REQ-1030
    When the feature condition is met
    Then damping force scales linearly with movement velocity

  @AC-1030.4
  Scenario: Damping operates within FFB safety force limits
    Given the system is configured for REQ-1030
    When the feature condition is met
    Then damping operates within ffb safety force limits
