Feature: Memory Safety
  As a flight simulation enthusiast
  I want memory safety
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: No unsafe code blocks exist outside FFI boundary modules
    Given the system is configured for memory safety
    When the feature is exercised
    Then no unsafe code blocks exist outside FFI boundary modules

  Scenario: FFI boundary modules document safety invariants for each unsafe block
    Given the system is configured for memory safety
    When the feature is exercised
    Then fFI boundary modules document safety invariants for each unsafe block

  Scenario: CI enforces unsafe audit via cargo-geiger or equivalent tooling
    Given the system is configured for memory safety
    When the feature is exercised
    Then cI enforces unsafe audit via cargo-geiger or equivalent tooling

  Scenario: All unsafe blocks are covered by miri tests where applicable
    Given the system is configured for memory safety
    When the feature is exercised
    Then all unsafe blocks are covered by miri tests where applicable
