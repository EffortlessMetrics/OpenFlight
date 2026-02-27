@REQ-342 @product
Feature: Automatic Profile Discovery  @AC-342.1
  Scenario: Service scans profile directory on startup
    Given the profile directory contains three YAML profile files
    When the service starts
    Then all three profiles SHALL be loaded and available for selection  @AC-342.2
  Scenario: New profile files are picked up without restart
    Given the service is running and a file watcher is active on the profile directory
    When a new YAML profile file is added to the profile directory
    Then the service SHALL detect and load the new profile without requiring a restart  @AC-342.3
  Scenario: Profile files with syntax errors are logged and skipped
    Given the profile directory contains a YAML file with a syntax error
    When the service scans the directory
    Then the service SHALL log the parse error for the invalid file and continue loading the remaining valid profiles  @AC-342.4
  Scenario: Profile names are derived from filenames
    Given a profile file named "a320neo.yaml" in the profile directory
    When the service loads the profile
    Then the profile SHALL be registered under the name "a320neo"  @AC-342.5
  Scenario: Profile discovery results are available via CLI list
    Given three profiles have been discovered and loaded by the service
    When the user runs "flightctl profile list"
    Then the command output SHALL include all three discovered profile names  @AC-342.6
  Scenario: Profile directory path is configurable
    Given the service configuration specifies a custom profile directory path
    When the service starts
    Then the service SHALL scan the custom directory instead of the default ~/.config/openflight/profiles/ path
