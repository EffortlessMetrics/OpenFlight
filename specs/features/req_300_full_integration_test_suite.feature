@REQ-300 @product
Feature: Full Integration Test Suite  @AC-300.1
  Scenario: CI runs an integration test exercising the full axis pipeline end-to-end
    Given the CI environment with a device simulator configured
    When the integration test suite is invoked
    Then the test SHALL exercise the path from device simulator through axis pipeline through bus publish to subscriber verification  @AC-300.2
  Scenario: Test validates axis values at each pipeline stage
    Given an integration test with known input axis values injected by the device simulator
    When the test runs
    Then axis values SHALL be asserted at the input, post-processing, and bus-publish stages of the pipeline  @AC-300.3
  Scenario: Test runs in under 30 seconds
    Given the full integration test suite
    When the suite is executed on the CI runner
    Then the total wall-clock runtime SHALL be less than 30 seconds  @AC-300.4
  Scenario: Test uses deterministic timing via mock clock
    Given the integration test is configured to use a mock clock
    When time-dependent pipeline logic executes
    Then all timing decisions SHALL be driven by the mock clock ensuring deterministic and reproducible results  @AC-300.5
  Scenario: Test failure produces a readable diff of expected vs actual axis values
    Given an integration test where the actual axis output does not match the expected output
    When the test framework reports the failure
    Then the failure output SHALL include a structured diff showing the expected and actual axis values at each stage  @AC-300.6
  Scenario: Test includes device simulator to axis pipeline to bus publish to subscriber verify
    Given the integration test harness is initialized
    When a simulated device event is injected
    Then the test SHALL verify that the event propagates through every stage: device simulator, axis pipeline, bus publish, and subscriber receipt
