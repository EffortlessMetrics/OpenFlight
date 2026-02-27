Feature: Service Plugin Isolation
  As a flight simulation enthusiast
  I want plugins to run in isolated sandboxes
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Plugins run in sandboxes
    Given a plugin is loaded by the service
    When the plugin starts
    Then it runs in an isolated sandbox with declared capabilities

  Scenario: Plugin crash does not affect host
    Given a plugin is running in a sandbox
    When the plugin crashes
    Then the host service continues running unaffected

  Scenario: Resource usage is bounded
    Given a plugin is running
    When it attempts to exceed its configured resource limits
    Then the usage is capped at the configured bounds

  Scenario: Communication via IPC protocol
    Given a plugin needs to communicate with the service
    When it sends a message
    Then communication uses the defined IPC protocol
