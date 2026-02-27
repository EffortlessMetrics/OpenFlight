Feature: CLI Status Dashboard
  As a flight simulation enthusiast
  I want cli status dashboard
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Dashboard with --dashboard flag
    Given the system is configured for cli status dashboard
    When the feature is exercised
    Then cli shows a rich status dashboard with the --dashboard flag

  Scenario: Device axis and health display
    Given the system is configured for cli status dashboard
    When the feature is exercised
    Then dashboard displays device, axis, and service health in real time

  Scenario: Auto-refresh interval
    Given the system is configured for cli status dashboard
    When the feature is exercised
    Then dashboard auto-refreshes at a configurable interval

  Scenario: Degrade in non-TTY environments
    Given the system is configured for cli status dashboard
    When the feature is exercised
    Then dashboard gracefully degrades in non-tty environments
