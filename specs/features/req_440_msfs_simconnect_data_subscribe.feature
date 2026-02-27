@REQ-440 @product
Feature: MSFS SimConnect Data Subscribe — Subscribe to and Publish Sim Variable Snapshots

  @AC-440.1
  Scenario: Adapter subscribes to aircraft state variables on connect
    Given the SimConnect adapter is configured with a variable list
    When a connection to MSFS is established
    Then the adapter SHALL register data definitions for all configured aircraft state variables

  @AC-440.2
  Scenario: Variable updates are converted to BusSnapshot and published at sim rate
    Given the SimConnect adapter is connected and subscribed
    When the simulator delivers a data frame
    Then the adapter SHALL convert it to a BusSnapshot and publish it to the event bus

  @AC-440.3
  Scenario: Subscription handles missing variables gracefully with default values
    Given a configured variable is not available in the current simulator version
    When the adapter receives a data frame without that variable
    Then it SHALL substitute the configured default value and continue publishing

  @AC-440.4
  Scenario: Adapter re-subscribes after SimConnect reconnection
    Given the SimConnect connection has dropped and reconnected
    When the adapter detects the reconnection
    Then it SHALL re-register all data definitions and resume publishing snapshots
