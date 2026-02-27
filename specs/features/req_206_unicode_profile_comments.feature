@REQ-206 @product
Feature: Profile TOML supports Unicode comments for internationalization  @AC-206.1
  Scenario: Profile comment fields accept UTF-8 strings
    Given a profile with a comment field set to a UTF-8 string
    When the profile is validated
    Then the comment field SHALL be accepted without error  @AC-206.2
  Scenario: Non-ASCII characters preserved through save and load cycle
    Given a profile containing non-ASCII characters in comment fields
    When the profile is saved and then loaded
    Then the comment fields SHALL contain the original non-ASCII characters unchanged  @AC-206.3
  Scenario: Japanese Arabic and Cyrillic scripts survive profile round-trip
    Given a profile with comments written in Japanese, Arabic, and Cyrillic scripts
    When the profile is serialised to TOML and deserialised
    Then all three scripts SHALL be present and correct in the loaded profile  @AC-206.4
  Scenario: Cloud sync preserves non-ASCII profile comments
    Given a profile containing non-ASCII comments is synced to the cloud
    When the profile is fetched back from cloud storage
    Then the non-ASCII comment text SHALL be identical to the original  @AC-206.5
  Scenario: CLI displays non-ASCII profile comments correctly in terminal
    Given a profile with non-ASCII comments is loaded
    When the user runs flightctl profile show
    Then the CLI SHALL display the non-ASCII comment text without corruption  @AC-206.6
  Scenario: Malformed UTF-8 in profile rejected at load with descriptive error
    Given a profile file containing malformed UTF-8 byte sequences in a comment field
    When the profile is loaded
    Then the load SHALL fail with a descriptive error identifying the malformed UTF-8
