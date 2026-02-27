Feature: Axis Engine Warm-Up Period
  As a flight simulation enthusiast
  I want axis engine warm-up period
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Discard input during warm-up
    Given the system is configured for axis engine warm-up period
    When the feature is exercised
    Then axis engine discards input during configurable warm-up ticks after startup

  Scenario: Default warm-up tick count
    Given the system is configured for axis engine warm-up period
    When the feature is exercised
    Then warm-up tick count defaults to 50 ticks (200ms at 250hz)

  Scenario: Configurable warm-up period
    Given the system is configured for axis engine warm-up period
    When the feature is exercised
    Then warm-up period is configurable via profile

  Scenario: Neutral output during warm-up
    Given the system is configured for axis engine warm-up period
    When the feature is exercised
    Then axis output is held at neutral during warm-up
