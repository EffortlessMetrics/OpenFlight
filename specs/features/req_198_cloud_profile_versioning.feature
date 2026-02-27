@REQ-198 @product
Feature: Cloud profiles are versioned and changes are tracked  @AC-198.1
  Scenario: Profile version assigned a monotonically increasing number
    Given a cloud-synced profile is modified and saved
    When the profile is saved to the cloud
    Then the new version SHALL receive a version number higher than the previous version  @AC-198.2
  Scenario: Previous 10 versions retained locally after cloud sync
    Given a cloud profile has been updated more than 10 times
    When a sync completes
    Then the 10 most recent versions SHALL be retained locally  @AC-198.3
  Scenario: Profile diff shows changed axes and mappings
    Given two retained versions of a cloud profile
    When a diff is requested between those versions
    Then the output SHALL identify which axes and mappings changed between the two versions  @AC-198.4
  Scenario: Rollback to any retained version via CLI
    Given multiple locally retained profile versions exist
    When the user issues a rollback command specifying a version number
    Then the profile SHALL be restored to that version  @AC-198.5
  Scenario: Version metadata includes timestamp, device list, and sim target
    Given a versioned cloud profile
    When version metadata is queried
    Then the metadata SHALL include the save timestamp, device list, and sim target for that version  @AC-198.6
  Scenario: Conflict between local and cloud version prompts user to choose
    Given a local profile modification and a different cloud profile version exist simultaneously
    When a sync is attempted
    Then the user SHALL be prompted to choose which version to keep
