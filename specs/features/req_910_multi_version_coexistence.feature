Feature: Multi-Version Coexistence
  As a flight simulation enthusiast
  I want multi-version coexistence
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Stable and beta versions install to separate directories without conflict
    Given the system is configured for multi-version coexistence
    When the feature is exercised
    Then stable and beta versions install to separate directories without conflict

  Scenario: Each version uses isolated configuration and data directories
    Given the system is configured for multi-version coexistence
    When the feature is exercised
    Then each version uses isolated configuration and data directories

  Scenario: IPC ports are version-namespaced to prevent cross-version interference
    Given the system is configured for multi-version coexistence
    When the feature is exercised
    Then iPC ports are version-namespaced to prevent cross-version interference

  Scenario: Only one version may be actively running at any given time
    Given the system is configured for multi-version coexistence
    When the feature is exercised
    Then only one version may be actively running at any given time
