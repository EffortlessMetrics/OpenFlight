@REQ-316 @product
Feature: Profile Inheritance Validation  @AC-316.1
  Scenario: Profile parser validates parent profile references on load
    Given a child profile that declares a parent profile reference
    When the profile is loaded
    Then the parser SHALL validate that the referenced parent profile exists and is resolvable  @AC-316.2
  Scenario: Circular inheritance is detected and rejected with a clear error
    Given a set of profiles where profile A inherits from B and B inherits from A
    When the profiles are loaded
    Then the service SHALL detect the circular inheritance and reject it with a descriptive error message  @AC-316.3
  Scenario: Inherited settings can be overridden at child level
    Given a child profile that overrides a setting defined in its parent
    When the merged profile is resolved
    Then the child profile's value SHALL take precedence over the parent's value for that setting  @AC-316.4
  Scenario: Settings not in child profile fall through to parent
    Given a child profile that does not define a setting present in its parent
    When the merged profile is resolved
    Then the parent profile's value SHALL be used for settings absent from the child profile  @AC-316.5
  Scenario: Maximum inheritance depth is 5 levels by default but configurable
    Given a profile chain deeper than 5 levels of inheritance
    When the profile is loaded with the default configuration
    Then the service SHALL reject the profile with an error indicating maximum inheritance depth exceeded  @AC-316.6
  Scenario: Inheritance chain is logged on profile load
    Given a valid profile with a parent chain of two or more levels
    When the profile is successfully loaded
    Then the service SHALL log the full inheritance chain at debug level during profile load
