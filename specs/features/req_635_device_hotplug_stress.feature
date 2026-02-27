Feature: Device Hotplug Stability Test
  As a flight simulation developer
  I want device hotplug to be stress tested for stability
  So that I can be confident the service handles repeated plug/unplug cycles without degradation

  Background:
    Given the OpenFlight service is running

  Scenario: Hotplug test connects and disconnects device 100 times
    Given the hotplug stress test is configured for 100 iterations
    When the hotplug stress test runs to completion
    Then exactly 100 connect and disconnect cycles are recorded

  Scenario: Service remains stable after all iterations
    Given the hotplug stress test has completed 100 iterations
    Then the service is still running and accepting commands

  Scenario: Memory usage does not grow with repeated hotplug
    When memory usage is measured before and after 100 hotplug cycles
    Then the memory delta is within acceptable bounds

  Scenario: Hotplug test is in the CI integration test suite
    When the CI integration test suite is executed
    Then the hotplug stability test is included in the run
