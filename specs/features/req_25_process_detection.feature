@REQ-25
Feature: Simulator process detection lifecycle

  @AC-25.1
  Scenario: Process detector initializes with known simulator definitions
    Given a newly created ProcessDetector
    When the list of registered process definitions is queried
    Then definitions for MSFS, X-Plane, and DCS SHALL be present
    And each definition SHALL have a non-empty process name pattern

  @AC-25.1
  Scenario: Process definition defaults are valid
    Given a default ProcessDefinition for a supported simulator
    When its fields are inspected
    Then the check interval SHALL have a positive duration
    And the detection threshold SHALL be greater than zero

  @AC-25.2
  Scenario: Process detection lifecycle detects running simulator
    Given a ProcessDetector with a mock process list containing a known simulator
    When the detection lifecycle is run
    Then the detector SHALL transition to the detected state for that simulator

  @AC-25.2
  Scenario: Sim detection check updates state correctly
    Given a ProcessDetector in the idle state
    When a sim detection check is performed with an active simulator in the process list
    Then the detector SHALL report the simulator as active
