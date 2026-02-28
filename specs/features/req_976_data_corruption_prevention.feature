Feature: Data Corruption Prevention
  As a flight simulation enthusiast
  I want data corruption prevention
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Configuration writes use transactional semantics with atomic file replacement
    Given the system is configured for data corruption prevention
    When the feature is exercised
    Then configuration writes use transactional semantics with atomic file replacement

  Scenario: Write operations create backup before modifying existing configuration
    Given the system is configured for data corruption prevention
    When the feature is exercised
    Then write operations create backup before modifying existing configuration

  Scenario: Corrupted configuration files are detected and replaced with last known good
    Given the system is configured for data corruption prevention
    When the feature is exercised
    Then corrupted configuration files are detected and replaced with last known good

  Scenario: File integrity checksums are validated on every configuration load
    Given the system is configured for data corruption prevention
    When the feature is exercised
    Then file integrity checksums are validated on every configuration load