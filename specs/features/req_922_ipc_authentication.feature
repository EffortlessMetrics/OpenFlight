Feature: IPC Authentication
  As a flight simulation enthusiast
  I want ipc authentication
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: gRPC connections require mutual TLS authentication between client and service
    Given the system is configured for ipc authentication
    When the feature is exercised
    Then gRPC connections require mutual TLS authentication between client and service

  Scenario: Authentication tokens are rotated on each service restart
    Given the system is configured for ipc authentication
    When the feature is exercised
    Then authentication tokens are rotated on each service restart

  Scenario: Unauthenticated IPC connections are rejected with appropriate error code
    Given the system is configured for ipc authentication
    When the feature is exercised
    Then unauthenticated IPC connections are rejected with appropriate error code

  Scenario: Authentication credentials are stored in platform-specific secure storage
    Given the system is configured for ipc authentication
    When the feature is exercised
    Then authentication credentials are stored in platform-specific secure storage
