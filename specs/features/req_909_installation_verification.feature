Feature: Installation Verification
  As a flight simulation enthusiast
  I want installation verification
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Post-install self-test verifies all binaries are present and executable
    Given the system is configured for installation verification
    When the feature is exercised
    Then post-install self-test verifies all binaries are present and executable

  Scenario: Self-test validates HID device access permissions are correctly configured
    Given the system is configured for installation verification
    When the feature is exercised
    Then self-test validates HID device access permissions are correctly configured

  Scenario: Self-test checks that sim plugin files are installed in correct locations
    Given the system is configured for installation verification
    When the feature is exercised
    Then self-test checks that sim plugin files are installed in correct locations

  Scenario: Self-test reports results in structured format for automated validation
    Given the system is configured for installation verification
    When the feature is exercised
    Then self-test reports results in structured format for automated validation
