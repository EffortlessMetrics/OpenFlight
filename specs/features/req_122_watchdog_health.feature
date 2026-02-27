@REQ-122 @infra
Feature: Watchdog health monitoring

  @AC-122.1
  Scenario: Healthy component passes watchdog check
    Given a watchdog monitor with a registered component
    When the component sends a heartbeat within the expected interval
    Then the watchdog SHALL report the component as healthy

  @AC-122.2
  Scenario: Consecutive failures trigger quarantine
    Given a watchdog monitor with a registered component
    When the component misses the heartbeat threshold number of consecutive checks
    Then the watchdog SHALL place the component into quarantine
    And a ComponentQuarantined event SHALL be emitted

  @AC-122.3
  Scenario: Recovery attempt clears quarantine
    Given a watchdog monitor with a component currently in quarantine
    When the component successfully responds to a recovery probe
    Then the watchdog SHALL clear the quarantine state for that component
    And the component SHALL be reported as healthy

  @AC-122.4
  Scenario: Component re-registers cleanly after unregister
    Given a watchdog monitor with a registered component
    When the component is unregistered
    And then re-registered with the same identifier
    Then the watchdog SHALL track the component as healthy from a clean state
    And no stale quarantine state SHALL carry over from the previous registration

  @AC-122.5
  Scenario: Multiple component types monitored independently
    Given a watchdog monitor with three components of different types registered
    When one component misses its heartbeat threshold
    Then only that component SHALL be quarantined
    And the other two components SHALL remain healthy

  @AC-122.6
  Scenario: Fault injection can be toggled off
    Given a watchdog monitor with fault injection enabled for a component
    When fault injection is disabled for that component
    Then the component SHALL no longer be artificially failed by the watchdog
    And subsequent heartbeats from that component SHALL be processed normally

  @AC-122.7
  Scenario: Health summary reflects current state
    Given a watchdog monitor with five registered components, two of which are quarantined
    When the health summary is requested
    Then the summary SHALL report five total components
    And the summary SHALL report two quarantined components
    And the summary SHALL report three healthy components

  @AC-122.8
  Scenario: Quarantined count never exceeds registered count
    Given a watchdog monitor under any combination of register, unregister, and failure events
    When the health summary is inspected at any point
    Then the quarantined component count SHALL never exceed the total registered component count
