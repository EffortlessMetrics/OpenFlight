Feature: Service Plugin Discovery
  As a flight simulation enthusiast
  I want service plugin discovery
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Plugins are scanned and loaded from configured directories at startup
    Given the system is configured for service plugin discovery
    When the feature is exercised
    Then plugins are scanned and loaded from configured directories at startup

  Scenario: Plugin manifests are validated before loading proceeds
    Given the system is configured for service plugin discovery
    When the feature is exercised
    Then plugin manifests are validated before loading proceeds

  Scenario: Incompatible or unsigned plugins are skipped with a warning
    Given the system is configured for service plugin discovery
    When the feature is exercised
    Then incompatible or unsigned plugins are skipped with a warning

  Scenario: Discovered plugins are listed in the service status report
    Given the system is configured for service plugin discovery
    When the feature is exercised
    Then discovered plugins are listed in the service status report
