Feature: CLI Remote Connection
  As a flight simulation enthusiast
  I want cli remote connection
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: CLI can connect to a remote service instance by address and port
    Given the system is configured for cli remote connection
    When the feature is exercised
    Then cLI can connect to a remote service instance by address and port

  Scenario: Remote connections require authentication via token or certificate
    Given the system is configured for cli remote connection
    When the feature is exercised
    Then remote connections require authentication via token or certificate

  Scenario: Connection timeout and retry are configurable per remote target
    Given the system is configured for cli remote connection
    When the feature is exercised
    Then connection timeout and retry are configurable per remote target

  Scenario: Remote connection status is shown in the CLI prompt or status bar
    Given the system is configured for cli remote connection
    When the feature is exercised
    Then remote connection status is shown in the CLI prompt or status bar
