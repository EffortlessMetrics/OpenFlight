@REQ-99 @product
Feature: Software Update Rollback, Startup Crash Detection, and Delta Patch Application

  Background:
    Given the flight-updater crate with RollbackManager, StartupCrashDetector, DeltaApplier, and DeltaPatch

  @AC-99.1
  Scenario: Startup crash detector flags a version installed within the startup timeout window
    Given a StartupCrashDetector with a 5-second startup timeout
    And a version that was installed 2 seconds ago
    When check_startup_crash is called
    Then the detector SHALL indicate a potential startup crash

  @AC-99.1
  Scenario: Startup success mark prevents crash detection on subsequent starts
    Given an UpdateManager that has successfully started
    When mark_startup_success is called
    Then a subsequent check_startup_crash call SHALL NOT indicate a crash

  @AC-99.2
  Scenario: Rollback to previous restores the immediately preceding version
    Given a RollbackManager with versions ["2.0.0", "1.0.0"] recorded (newest first)
    When rollback_to_previous is called
    Then the returned VersionInfo SHALL have version "1.0.0"
    And the current_version SHALL be updated to "1.0.0"

  @AC-99.2
  Scenario: Rollback with only one version recorded returns an error
    Given a RollbackManager with only version "1.0.0" recorded
    When rollback_to_previous is called
    Then the result SHALL be Err with a "No previous version available" description

  @AC-99.3
  Scenario: Delta patch application round-trips binary content losslessly
    Given a DeltaPatch generated from source content "hello" to target content "hello world"
    When the patch is applied to the source
    Then the output SHALL equal "hello world" exactly

  @AC-99.3
  Scenario: Delta patch application rejects a source file whose hash does not match the patch manifest
    Given a DeltaPatch whose source_hash records hash H
    And source content whose actual SHA256 hash differs from H
    When the patch is applied
    Then the result SHALL be Err(DeltaPatch(source hash mismatch))

  @AC-99.4
  Scenario: Failed update leaves the running version unchanged
    Given an UpdateManager running version "1.5.0"
    When an update to "1.6.0" is attempted but fails mid-installation
    Then the current running version SHALL remain "1.5.0"
    And the update directory SHALL NOT contain a partial "1.6.0" installation that would block future updates

  @AC-99.4
  Scenario: Rollback after startup crash preserves the pre-update version on disk
    Given an UpdateManager that upgraded from "1.0.0" to "1.1.0" and then detected a startup crash
    When automatic rollback executes
    Then the installed version SHALL revert to "1.0.0"
    And "1.1.0" SHALL be recorded as the failed version
