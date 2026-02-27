@REQ-248 @infra
Feature: Update system fetches and applies updates from stable/beta/canary channels  @AC-248.1
  Scenario: Update manifest fetched from HTTPS endpoint on configurable schedule
    Given the updater is configured with an HTTPS manifest URL and a 24-hour check interval
    When the scheduled check interval elapses
    Then the updater SHALL fetch the manifest from the HTTPS endpoint and parse the available version information  @AC-248.2
  Scenario: Manifest signature verified before applying update
    Given a downloaded update manifest
    When the updater validates the manifest
    Then the cryptographic signature SHALL be verified against the trusted public key and an invalid signature SHALL cause the update to be rejected  @AC-248.3
  Scenario: Delta update downloads only changed files
    Given the current installation is one patch version behind the latest release
    When the updater fetches the update package
    Then only the files that differ between the two versions SHALL be downloaded, not the full binary  @AC-248.4
  Scenario: Rollback available for two previous versions
    Given the service has been updated twice since the initial installation
    When a rollback command is issued
    Then the updater SHALL be able to restore either of the two previous versions  @AC-248.5
  Scenario: Update channel configurable as stable beta or canary
    Given the updater configuration specifies channel beta
    When the updater fetches the manifest
    Then the manifest endpoint used SHALL correspond to the beta channel and stable-only releases SHALL not be offered  @AC-248.6
  Scenario: Update applied on service restart not mid-session
    Given a new update package has been downloaded and verified
    When the service is running an active flight session
    Then the update SHALL NOT be applied until the service is restarted after the session ends
