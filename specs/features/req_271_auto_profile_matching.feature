@REQ-271 @product
Feature: Service automatically matches profiles to detected simulator and aircraft type  @AC-271.1
  Scenario: Service detects running simulator automatically
    Given the flightd service is running and a supported simulator is launched
    When the simulator process becomes detectable
    Then the service SHALL identify the active simulator without any user action  @AC-271.2
  Scenario: Aircraft type triggers profile switch within 500 ms
    Given auto-matching is enabled and the service has detected the active simulator
    When the simulator reports a new aircraft type
    Then the service SHALL complete the profile switch within 500 ms of the aircraft type notification  @AC-271.3
  Scenario: Exact call-sign match takes priority over fuzzy match
    Given profiles exist for call-sign FA-18C and a fuzzy pattern matching FA-18
    When the simulator reports aircraft call-sign FA-18C
    Then the service SHALL select the exact FA-18C profile rather than the fuzzy-matched profile  @AC-271.4
  Scenario: Ambiguous fuzzy match selects most recently used profile
    Given two profiles both match the aircraft call-sign by fuzzy rules
    When the service must choose between them
    Then the service SHALL select the profile with the more recent last-used timestamp  @AC-271.5
  Scenario: Auto-matching disabled for current session via CLI
    Given auto-matching is currently enabled
    When the user runs flightctl match disable
    Then auto-matching SHALL be suspended for the session and the active profile SHALL not change on aircraft switch  @AC-271.6
  Scenario: Match events logged with aircraft and profile name
    Given auto-matching is enabled and a profile switch occurs
    When the match completes
    Then the service SHALL write a log entry containing both the detected aircraft name and the selected profile name
