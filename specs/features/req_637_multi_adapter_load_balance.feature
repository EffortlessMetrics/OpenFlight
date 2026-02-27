Feature: Multi-Adapter Load Balancing
  As a advanced OpenFlight user
  I want the service to distribute load when multiple adapters are active
  So that I can run multiple simulator adapters simultaneously without contention

  Background:
    Given the OpenFlight service is running

  Scenario: Multiple active adapters run on separate threads
    Given MSFS and X-Plane adapters are both configured and running
    When thread assignments are inspected
    Then each adapter runs on a separate thread

  Scenario: Thread priority is configurable per adapter
    Given an adapter thread priority is set in config
    When the adapter starts
    Then the thread runs at the configured priority

  Scenario: Adapter thread affinity can be pinned to specific CPUs
    Given CPU affinity is set for an adapter in config
    When the adapter thread starts
    Then the thread is pinned to the specified CPU cores

  Scenario: Load distribution is visible in service diagnostics
    Given multiple adapters are running
    When service diagnostics are queried
    Then the response includes per-adapter thread and CPU utilization
