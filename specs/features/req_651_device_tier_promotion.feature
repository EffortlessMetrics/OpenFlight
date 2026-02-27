Feature: Device Compatibility Tier Promotion
  As a flight simulation enthusiast
  I want device tier to be promotable based on automated test results
  So that device compatibility ratings are kept accurate and up to date

  Background:
    Given the device compatibility manifest is available

  Scenario: Passing HIL test promotes device to Tier 1
    Given a device has passed the hardware-in-loop (HIL) compatibility test
    When the tier promotion process runs
    Then the device is promoted to Tier 1 in the compatibility manifest

  Scenario: Automated trace test without HIL promotes to Tier 2
    Given a device has passed the automated trace test without HIL hardware
    When the tier promotion process runs
    Then the device is promoted to Tier 2 in the compatibility manifest

  Scenario: Tier promotion is recorded in compatibility manifest
    Given a device tier promotion has been determined
    When the manifest is updated
    Then the new tier and promotion timestamp are recorded in the manifest entry

  Scenario: Tier changes are logged in CI artifacts
    Given a CI pipeline run includes tier promotion checks
    When a device tier changes during the run
    Then the tier change is included in the CI artifact log output
