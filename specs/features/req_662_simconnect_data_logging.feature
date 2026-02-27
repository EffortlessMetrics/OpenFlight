Feature: SimConnect Data Logging
  As a flight simulation enthusiast
  I want the SimConnect adapter to support structured data logging
  So that I can capture and analyse simulator variable values over time

  Background:
    Given the OpenFlight service is running and connected to MSFS via SimConnect

  Scenario: SimConnect variable values are loggable to CSV on demand
    When a data logging session is started for selected SimConnect variables
    Then their values are written to a CSV file on demand

  Scenario: Logging is started and stopped via CLI
    When the user runs the start logging CLI command
    Then logging begins, and when the stop command is issued logging ceases

  Scenario: Log includes timestamp, variable name, and value
    Given a data logging session is active
    When log entries are written
    Then each entry contains a timestamp, the SimConnect variable name, and its value

  Scenario: Log rate is configurable up to sim frame rate
    When a log rate is configured for the data logging session
    Then the adapter samples variables at the configured rate up to the simulator frame rate
