@REQ-432 @product
Feature: Profile Hot-Reload — Reload Profiles Without Service Restart

  @AC-432.1
  Scenario: Service watches profile directory for file modification events
    Given the service is running with a profile directory configured
    When a profile file in that directory is modified on disk
    Then the service SHALL detect the change via filesystem watch events

  @AC-432.2
  Scenario: Modified profile is parsed and compiled off the RT thread
    Given a profile file change is detected
    When the hot-reload is triggered
    Then parsing and compilation SHALL occur on a non-RT thread without blocking the 250Hz spine

  @AC-432.3
  Scenario: New compiled profile is atomically swapped into the RT spine at tick boundary
    Given a successfully compiled new profile
    When the swap is performed
    Then it SHALL be applied atomically at the next RT tick boundary with no mid-tick partial state

  @AC-432.4
  Scenario: Hot-reload failure logs error and retains previous profile
    Given a modified profile file contains a parse error
    When the hot-reload attempt fails
    Then the service SHALL log an error, retain the previously active profile, and continue running
