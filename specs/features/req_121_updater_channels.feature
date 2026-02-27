@REQ-121 @product
Feature: Updater channel management

  @AC-121.1
  Scenario: Stable channel only returns non-prerelease versions
    Given an updater configured on the stable channel
    When the version manifest is fetched
    Then the list of available versions SHALL contain only non-prerelease versions
    And no alpha, beta, or canary versions SHALL appear

  @AC-121.2
  Scenario: Beta channel includes beta and stable versions
    Given an updater configured on the beta channel
    When the version manifest is fetched
    Then the list of available versions SHALL include stable versions
    And the list SHALL also include beta pre-release versions
    And alpha or canary versions SHALL NOT be included

  @AC-121.3
  Scenario: Canary channel includes all versions
    Given an updater configured on the canary channel
    When the version manifest is fetched
    Then the list of available versions SHALL include stable, beta, and canary versions

  @AC-121.4
  Scenario: Version comparison 1.2.3 greater than 1.2.2
    Given the version comparison function
    When version "1.2.3" is compared to version "1.2.2"
    Then "1.2.3" SHALL be considered greater than "1.2.2"
    And "1.2.2" SHALL be considered less than "1.2.3"

  @AC-121.5
  Scenario: Manifest checksum verification passes for valid file
    Given an update manifest with a known SHA-256 checksum
    When the downloaded file is verified against the manifest checksum
    And the file has not been modified
    Then the checksum verification SHALL pass

  @AC-121.6
  Scenario: Manifest checksum verification fails for tampered file
    Given an update manifest with a known SHA-256 checksum
    When the downloaded file has been modified after download
    Then the checksum verification SHALL fail
    And the updater SHALL reject the corrupted file

  @AC-121.7
  Scenario: Rollback restores previous version
    Given a system with version 1.2.3 installed and a rollback point at 1.2.2
    When a rollback is requested
    Then the installed version SHALL revert to 1.2.2
    And the rollback SHALL complete without error

  @AC-121.8
  Scenario: Update delta applies cleanly to base version
    Given a base installation at version 1.2.2
    And a delta patch targeting version 1.2.3
    When the delta patch is applied
    Then the resulting installation SHALL be functionally equivalent to a clean install of 1.2.3
