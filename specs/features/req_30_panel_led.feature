@REQ-30
Feature: Panel LED control and bytecode rule evaluation

  @AC-30.1
  Scenario: LED on and off modes apply correctly
    Given a panel LED driver
    When the LED is set to on and then off
    Then the output state SHALL reflect each command respectively

  @AC-30.1
  Scenario: LED blink mode applies at configured frequency
    Given a panel LED driver with blink support
    When blink mode is activated at a given frequency
    Then the LED output SHALL alternate at the specified frequency

  @AC-30.1
  Scenario: LED brightness can be set
    Given a panel LED driver supporting brightness
    When brightness is set to a specific level
    Then the output brightness SHALL match the requested level

  @AC-30.2
  Scenario: LED updates respect minimum interval
    Given a panel LED with a minimum update interval configured
    When multiple updates arrive faster than the minimum interval
    Then only one update SHALL be forwarded within each interval window

  @AC-30.3
  Scenario: Bytecode rule evaluates LED state correctly
    Given a bytecode program implementing a simple LED rule
    When the evaluator processes a matching telemetry snapshot
    Then the resulting LED action SHALL match the rule's expected output

  @AC-30.3
  Scenario: Bytecode evaluation produces zero allocations on hot path
    Given a compiled bytecode rule
    When the evaluator is run on the hot path
    Then no heap allocations SHALL occur during evaluation
