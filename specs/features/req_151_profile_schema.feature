@REQ-151 @product
Feature: Profile schema validation  @AC-151.1
  Scenario: Valid profile schema version 1 accepted
    Given a profile document declaring schema version 1
    When the profile validator processes the document
    Then the validator SHALL accept the profile without errors  @AC-151.2
  Scenario: Future schema version rejected
    Given a profile document declaring a schema version higher than the supported maximum
    When the profile validator processes the document
    Then the validator SHALL reject the profile with a schema version error  @AC-151.3
  Scenario: Deadzone 0.0 accepted
    Given a profile axis configuration with deadzone set to 0.0
    When the profile validator processes the document
    Then the validator SHALL accept the deadzone value  @AC-151.4
  Scenario: Deadzone 1.0 accepted
    Given a profile axis configuration with deadzone set to 1.0
    When the profile validator processes the document
    Then the validator SHALL accept the deadzone value  @AC-151.5
  Scenario: Deadzone greater than 1.0 rejected
    Given a profile axis configuration with deadzone set to 1.1
    When the profile validator processes the document
    Then the validator SHALL reject the profile with a deadzone range error  @AC-151.6
  Scenario: Expo outside allowed range rejected
    Given a profile axis configuration with expo set to -0.1
    When the profile validator processes the document
    Then the validator SHALL reject the profile with an expo range error  @AC-151.7
  Scenario: Empty profile with no axes is valid
    Given a profile document with no axis configurations
    When the profile validator processes the document
    Then the validator SHALL accept the empty profile without errors
