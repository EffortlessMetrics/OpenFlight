@REQ-197 @product
Feature: OpenFlight operates with multiple simulators running simultaneously  @AC-197.1
  Scenario: MSFS and X-Plane active with separate profiles
    Given MSFS and X-Plane are both running
    When OpenFlight loads sim-specific profiles for each
    Then each simulator SHALL receive its own independent profile configuration  @AC-197.2
  Scenario: Each sim adapter operates independently on the bus
    Given MSFS and X-Plane adapters are both active
    When each adapter processes its own telemetry stream
    Then each adapter SHALL publish events to the bus independently without interfering with the other  @AC-197.3
  Scenario: Device axis assignment per sim specified in profile
    Given a profile with per-sim device axis assignments
    When the profile is loaded for a multi-sim session
    Then each device axis SHALL be routed to the correct simulator as specified in the profile  @AC-197.4
  Scenario: RT spine processes all sims' outputs in a single 250Hz tick
    Given multiple sim adapters are active and publishing axis data
    When the RT spine executes a processing tick
    Then outputs from all active simulators SHALL be processed within a single 250Hz tick  @AC-197.5
  Scenario: Switching active sim does not require service restart
    Given OpenFlight is running with MSFS as the active sim
    When the user switches the active sim to X-Plane
    Then the transition SHALL complete without restarting the OpenFlight service  @AC-197.6
  Scenario: Conflict detection warns on same device bound to multiple sims
    Given a profile binding the same device to both MSFS and X-Plane
    When the profile is loaded
    Then OpenFlight SHALL emit a conflict warning identifying the double-bound device
