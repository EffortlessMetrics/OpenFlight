Feature: Certificate Pinning
  As a flight simulation enthusiast
  I want certificate pinning
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Update server TLS certificates are pinned in the application binary
    Given the system is configured for certificate pinning
    When the feature is exercised
    Then update server TLS certificates are pinned in the application binary

  Scenario: Certificate rotation is supported via signed pin update mechanism
    Given the system is configured for certificate pinning
    When the feature is exercised
    Then certificate rotation is supported via signed pin update mechanism

  Scenario: Pinning failure aborts connection and logs security event
    Given the system is configured for certificate pinning
    When the feature is exercised
    Then pinning failure aborts connection and logs security event

  Scenario: Backup pins allow recovery when primary certificate is rotated
    Given the system is configured for certificate pinning
    When the feature is exercised
    Then backup pins allow recovery when primary certificate is rotated
