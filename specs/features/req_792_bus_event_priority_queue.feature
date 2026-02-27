Feature: Bus Event Priority Queue
  As a flight simulation enthusiast
  I want bus event priority queue
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: High priority processed first
    Given the system is configured for bus event priority queue
    When the feature is exercised
    Then bus processes high-priority events before normal-priority events

  Scenario: Three priority levels
    Given the system is configured for bus event priority queue
    When the feature is exercised
    Then priority levels include high, normal, and low

  Scenario: FIFO within same priority
    Given the system is configured for bus event priority queue
    When the feature is exercised
    Then same-priority events maintain fifo order

  Scenario: No allocation on RT path
    Given the system is configured for bus event priority queue
    When the feature is exercised
    Then priority queue does not allocate on the rt path
