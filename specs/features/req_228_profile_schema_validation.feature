@REQ-228 @product
Feature: Profile files are validated against schema before loading  @AC-228.1
  Scenario: Profile must specify schema field at top level
    Given a profile file without a top-level schema field
    When the service attempts to load the profile
    Then loading SHALL be rejected with an error stating the schema field is required  @AC-228.2
  Scenario: Unknown top-level fields produce warning not error
    Given a profile file with an unrecognised top-level field
    When the service loads the profile
    Then a warning SHALL be logged for the unknown field and the profile SHALL still load successfully  @AC-228.3
  Scenario: Axis deadzone outside valid range rejected with descriptive error
    Given a profile with an axis deadzone value outside the range 0.0 to 1.0
    When the service attempts to load the profile
    Then loading SHALL be rejected with an error message that names the invalid field and the valid range  @AC-228.4
  Scenario: Axis curve control points must be monotonically increasing
    Given a profile with axis curve control points that are not monotonically increasing
    When the service attempts to load the profile
    Then loading SHALL be rejected with a descriptive error identifying the non-monotonic segment  @AC-228.5
  Scenario: Profile version migration applied automatically on schema version detection
    Given a profile file with an older schema version
    When the service loads the profile
    Then the service SHALL automatically migrate the profile to the current schema version before applying it  @AC-228.6
  Scenario: Validation errors include field path for debugging
    Given a profile file with a validation error in a nested field
    When the service attempts to load the profile
    Then the error message SHALL include the full field path such as axes.pitch.deadzone to identify the problem location
