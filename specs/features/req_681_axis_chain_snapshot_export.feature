Feature: Axis Chain Snapshot Export
  As a flight simulation enthusiast
  I want to export the current axis chain state as a snapshot
  So that I can back up and restore my axis pipeline configuration

  Background:
    Given the OpenFlight service is running

  Scenario: CLI command exports axis chain config as JSON
    Given an axis chain is active with configured pipeline stages
    When I run the axis chain snapshot export CLI command
    Then a JSON file is created containing the current axis chain configuration

  Scenario: Snapshot includes filter parameters and pipeline order
    Given an axis chain snapshot has been exported
    When the snapshot JSON is inspected
    Then it contains the filter parameters and the ordered list of pipeline stages

  Scenario: Snapshot can be imported to restore configuration
    Given an axis chain snapshot JSON file exists
    When I run the axis chain snapshot import CLI command with that file
    Then the axis chain is restored to the configuration described in the snapshot

  Scenario: Snapshot export includes timestamp and device info
    Given an axis chain snapshot has been exported
    When the snapshot JSON is inspected
    Then it contains the export timestamp and the associated device information
