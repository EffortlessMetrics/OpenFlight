@REQ-580 @product
Feature: Profile Lock Mode — Service should support locking the active profile against changes  @AC-580.1
  Scenario: flightctl profile lock prevents profile hot-reload
    Given the profile is locked via flightctl profile lock
    When a profile file change is detected on disk
    Then the service SHALL not hot-reload the profile  @AC-580.2
  Scenario: Lock state survives service restart
    Given the profile lock is active
    When the service is restarted
    Then the profile SHALL remain locked after the restart  @AC-580.3
  Scenario: Lock can be bypassed by elevated operations
    Given the profile is locked
    When an elevated operation such as flightctl profile force-reload is run
    Then the profile lock SHALL be bypassed and the profile SHALL be reloaded  @AC-580.4
  Scenario: Lock state is visible in flightctl status
    Given the profile lock is active
    When the user runs flightctl status
    Then the output SHALL indicate that the profile is locked
