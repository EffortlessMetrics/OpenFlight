@REQ-214 @product
Feature: Profile selection automatically switches based on active simulator and aircraft  @AC-214.1
  Scenario: Profile auto-selected when new aircraft detected from any connected sim
    Given a simulator connected and an aircraft profile defined for a known ICAO code
    When the simulator reports a new aircraft with that ICAO code
    Then the matching profile SHALL be activated automatically  @AC-214.2
  Scenario: Profile hierarchy aircraft-specific then sim-specific then global default
    Given profiles defined at aircraft-specific, sim-specific, and global default levels
    When an aircraft is detected that matches all three levels
    Then the aircraft-specific profile SHALL take precedence over sim-specific and global default  @AC-214.3
  Scenario: Auto-switch event logged with aircraft ICAO sim and profile applied
    Given auto-profile switching is enabled
    When a profile switch is triggered by aircraft detection
    Then a log entry SHALL be written containing the aircraft ICAO code, simulator name, and profile applied  @AC-214.4
  Scenario: User-locked profile prevents auto-switch until unlocked
    Given the user has locked a specific profile
    When a new aircraft is detected that would normally trigger an auto-switch
    Then the locked profile SHALL remain active and no auto-switch SHALL occur  @AC-214.5
  Scenario: No matching profile falls back to global default cleanly
    Given an aircraft is detected with no aircraft-specific or sim-specific profile
    When the profile resolution runs
    Then the global default profile SHALL be applied without error  @AC-214.6
  Scenario: Auto-switch latency less than 500ms from aircraft detection to profile active
    Given auto-profile switching is enabled
    When an aircraft detection event is received
    Then the new profile SHALL be fully active within 500 milliseconds of the detection event
