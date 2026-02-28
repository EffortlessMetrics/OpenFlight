Feature: Background Download
  As a flight simulation enthusiast
  I want background download
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Updates download in background without interrupting active flight session
    Given the system is configured for background download
    When the feature is exercised
    Then updates download in background without interrupting active flight session

  Scenario: Download uses bandwidth throttling to avoid impacting sim network traffic
    Given the system is configured for background download
    When the feature is exercised
    Then download uses bandwidth throttling to avoid impacting sim network traffic

  Scenario: Partial downloads resume from last checkpoint after network interruption
    Given the system is configured for background download
    When the feature is exercised
    Then partial downloads resume from last checkpoint after network interruption

  Scenario: Download progress is reported via IPC for UI consumption
    Given the system is configured for background download
    When the feature is exercised
    Then download progress is reported via IPC for UI consumption
