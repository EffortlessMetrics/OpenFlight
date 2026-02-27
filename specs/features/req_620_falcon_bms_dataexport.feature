Feature: Falcon BMS DataExport Support
  As a flight simulation enthusiast
  I want OpenFlight to support Falcon BMS via its DataExport UDP protocol
  So that I can use my flight controls with Falcon BMS

  Background:
    Given the OpenFlight service is running

  Scenario: Falcon BMS DataExport telemetry is received on UDP port 1234
    Given the Falcon BMS DataExport adapter is enabled
    When Falcon BMS sends telemetry on UDP port 1234
    Then the adapter receives and parses the DataExport packets

  Scenario: Flight dynamics variables are extracted and normalized
    Given the Falcon BMS DataExport adapter is receiving telemetry
    When a DataExport packet is processed
    Then flight dynamics variables are extracted and normalized to standard ranges

  Scenario: Falcon BMS is listed in compatibility matrix
    When the compatibility matrix is inspected
    Then Falcon BMS is listed as a supported simulator

  Scenario: BMS adapter handles reconnection when sim restarts
    Given the Falcon BMS DataExport adapter has an active connection
    When Falcon BMS restarts and resumes sending telemetry
    Then the adapter reconnects and resumes data processing without service restart
