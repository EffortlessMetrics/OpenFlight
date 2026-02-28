Feature: Security Audit Automation
  As a flight simulation enthusiast
  I want security audit automation
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Automated security scanning runs as part of the CI pipeline
    Given the system is configured for security audit automation
    When the feature is exercised
    Then automated security scanning runs as part of the CI pipeline

  Scenario: Dependency vulnerabilities are checked with cargo-audit and cargo-deny
    Given the system is configured for security audit automation
    When the feature is exercised
    Then dependency vulnerabilities are checked with cargo-audit and cargo-deny

  Scenario: Audit failures block the merge pipeline with a clear report
    Given the system is configured for security audit automation
    When the feature is exercised
    Then audit failures block the merge pipeline with a clear report

  Scenario: False positives can be suppressed with a reviewed allow-list
    Given the system is configured for security audit automation
    When the feature is exercised
    Then false positives can be suppressed with a reviewed allow-list
