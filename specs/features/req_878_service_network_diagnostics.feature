Feature: Service Network Diagnostics
  As a flight simulation enthusiast
  I want service network diagnostics
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Network connectivity to each sim adapter endpoint is tested on demand
    Given the system is configured for service network diagnostics
    When the feature is exercised
    Then network connectivity to each sim adapter endpoint is tested on demand

  Scenario: Diagnostic reports latency and packet loss per adapter connection
    Given the system is configured for service network diagnostics
    When the feature is exercised
    Then diagnostic reports latency and packet loss per adapter connection

  Scenario: Unreachable adapters are flagged in the service health status
    Given the system is configured for service network diagnostics
    When the feature is exercised
    Then unreachable adapters are flagged in the service health status

  Scenario: Network diagnostic results are cached to avoid repeated probes
    Given the system is configured for service network diagnostics
    When the feature is exercised
    Then network diagnostic results are cached to avoid repeated probes
