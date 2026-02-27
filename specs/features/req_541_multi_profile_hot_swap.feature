Feature: Multi-Profile Hot-Swap
  As a flight simulation enthusiast
  I want to instantly switch between pre-loaded profiles
  So that I can change configurations without any perceptible delay

  Background:
    Given the OpenFlight service is running
    And profiles "combat" and "cruise" are pre-loaded into memory

  Scenario: Instant profile swap via CLI
    Given the active profile is "cruise"
    When I run "flightctl profile swap combat"
    Then the active profile becomes "combat"
    And the swap completes without dropping any axis ticks

  Scenario: Profile swap completes atomically within one axis tick
    Given the active profile is "combat"
    When the profile hot-swap RPC is called with profile "cruise"
    Then the swap is applied at the next axis tick boundary
    And no intermediate state is observable by the axis pipeline

  Scenario: Swap latency is under 1 millisecond
    When I trigger 100 consecutive profile swaps between "combat" and "cruise"
    Then the p99 swap latency is under 1 millisecond
