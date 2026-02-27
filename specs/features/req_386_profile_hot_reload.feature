@REQ-386 @product
Feature: Profile Hot-Reload Without Service Restart  @AC-386.1
  Scenario: File watcher detects changes to the active profile file
    Given the service is running with a profile loaded from a file on disk
    When the profile file is modified on disk
    Then the service SHALL detect the change via the file watcher  @AC-386.2
  Scenario: Changed profile is parsed and validated before applying
    Given a file change has been detected for the active profile
    When the new profile file is loaded
    Then it SHALL be fully parsed and validated before being applied to the RT spine  @AC-386.3
  Scenario: Invalid profile leaves the old profile active and logs an error
    Given the profile file is replaced with an invalid document
    When the hot-reload attempts to apply the new profile
    Then the old profile SHALL remain active and an error SHALL be logged  @AC-386.4
  Scenario: Hot-reload triggers within 500 ms of file change
    Given the service is running with file watching enabled
    When the profile file is saved with new contents
    Then the new profile SHALL be applied within 500 ms  @AC-386.5
  Scenario: Hot-reload is atomic at tick boundaries
    Given the RT spine is processing ticks during a profile hot-reload
    When the profile swap occurs
    Then each tick SHALL observe either the complete old or the complete new profile  @AC-386.6
  Scenario: flightctl profile reload forces immediate reload without file change
    Given the service is running with a loaded profile
    When the user runs flightctl profile reload
    Then the profile file SHALL be re-read and applied immediately
