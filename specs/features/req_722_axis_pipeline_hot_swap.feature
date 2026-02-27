Feature: Axis Pipeline Hot-Swap
  As a flight simulation enthusiast
  I want axis pipeline stages to be swappable at runtime
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Swap without stopping engine
    Given the axis engine is running
    When a pipeline stage swap is requested
    Then the stage is swapped without stopping the engine

  Scenario: Atomic swap at tick boundaries
    Given a pipeline swap is in progress
    When the current tick completes
    Then the swap is applied atomically with no dropped frames

  Scenario: New pipeline validated before swap
    Given a new pipeline configuration is submitted
    When the configuration is received
    Then it is validated before the swap is committed

  Scenario: Failed swap rolls back
    Given a pipeline swap fails validation
    When the swap is rejected
    Then the previous pipeline configuration is restored
