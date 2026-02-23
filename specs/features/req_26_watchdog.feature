@REQ-26
Feature: System watchdog USB stall, plugin overrun, and NaN detection

  @AC-26.1
  Scenario: USB stall detection triggers at threshold
    Given a watchdog with a USB stall threshold of 3
    When 3 or more USB stall events are recorded
    Then the watchdog SHALL report a stall condition for the endpoint

  @AC-26.1
  Scenario: USB stall counter resets on recovery
    Given a watchdog reporting a USB stall
    When the stall counter is reset
    Then the watchdog SHALL report the endpoint as no longer stalled

  @AC-26.2
  Scenario: Plugin overrun triggers quarantine at threshold
    Given a watchdog with a plugin overrun quarantine threshold
    When the overrun count exceeds the threshold for a plugin
    Then the plugin SHALL be placed in quarantine

  @AC-26.2
  Scenario: Plugin overrun detection records overrun events
    Given a watchdog monitoring a WASM plugin
    When a budget overrun is recorded for the plugin
    Then the overrun event SHALL be logged in the plugin's health record

  @AC-26.3
  Scenario: NaN guard detects injected NaN values
    Given a watchdog with NaN guard enabled
    When a NaN value is injected into a monitored channel
    Then the NaN guard SHALL detect the violation

  @AC-26.3
  Scenario: NaN guard can be disabled
    Given a watchdog with NaN guard disabled
    When a NaN value is injected
    Then no NaN violation SHALL be reported

  @AC-26.3
  Scenario: Critical component NaN triggers safety response
    Given a watchdog monitoring a critical RT component
    When a NaN value is detected in that component
    Then the watchdog SHALL trigger a safety response for that component

  @AC-26.4
  Scenario: Quarantined component can recover
    Given a component that was placed in quarantine
    When sufficient recovery time elapses and health checks pass
    Then the watchdog SHALL allow the component to leave quarantine

  @AC-26.4
  Scenario: Quarantine isolates component from system
    Given a component in quarantine
    When the system checks whether to route data through that component
    Then the component SHALL be bypassed while quarantined
