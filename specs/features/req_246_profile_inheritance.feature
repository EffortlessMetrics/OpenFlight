@REQ-246 @product
Feature: Child profiles inherit settings from parent profiles via cascade  @AC-246.1
  Scenario: Global to Simulator to Aircraft to Phase-of-Flight cascade applies in order
    Given profiles defined at Global, Simulator, Aircraft, and Phase-of-Flight levels
    When the profile merge pipeline runs
    Then settings SHALL be applied in cascade order with each level overriding the previous  @AC-246.2
  Scenario: Child profile overrides individual axis settings from parent
    Given a global profile with aileron sensitivity 0.5 and an aircraft profile overriding it to 0.8
    When the merged profile is computed
    Then the effective aileron sensitivity SHALL be 0.8  @AC-246.3
  Scenario: Explicit null in child profile resets axis to default
    Given a global profile with rudder curve set to a custom value and an aircraft profile setting rudder curve to null
    When the merged profile is computed
    Then the effective rudder curve SHALL be the system default, not the global value  @AC-246.4
  Scenario: Merge result logged at DEBUG level
    Given DEBUG logging is enabled
    When the profile merge pipeline produces a merged result
    Then each axis setting in the merged profile SHALL be recorded in the service log at DEBUG level  @AC-246.5
  Scenario: Circular inheritance detected and rejected with error
    Given two profiles each declaring the other as their parent
    When the profile loader attempts to resolve the hierarchy
    Then loading SHALL be rejected with an explicit circular inheritance error before any profile is applied  @AC-246.6
  Scenario: Profile tree validated at load time not at runtime
    Given a profile file with an invalid inheritance reference
    When the service loads its configuration at startup
    Then the validation error SHALL be reported at load time and the service SHALL refuse to start with the invalid profile
