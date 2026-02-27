Feature: Axis Engine Thread Safety Guarantee
  As a flight simulation developer
  I want the axis engine public API to document thread safety guarantees
  So that I can safely use it from multiple threads

  Background:
    Given the flight-axis crate is available

  Scenario: All Send and Sync bounds are documented in crate rustdoc
    When the crate rustdoc is inspected
    Then all public types document their Send and Sync bounds

  Scenario: Non-thread-safe types are explicitly marked with explanation
    When a public type does not implement Send or Sync
    Then its rustdoc includes an explanation of why it is not thread-safe

  Scenario: Panic conditions are documented for all public functions
    When a public function can panic
    Then its rustdoc includes a Panics section describing the conditions

  Scenario: Thread safety tests exist for concurrent access patterns
    When the test suite is run
    Then tests exist that exercise concurrent access to axis engine public APIs
