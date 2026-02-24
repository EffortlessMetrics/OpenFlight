@REQ-23
Feature: Profile management and hierarchical merging

  @AC-23.1
  Scenario: Profile validation rejects invalid configurations
    Given a profile with an invalid axis configuration
    When the profile is validated
    Then validation SHALL return an error identifying the invalid field

  @AC-23.1
  Scenario: Profile validation accepts valid configurations
    Given a well-formed profile with valid axis and button mappings
    When the profile is validated
    Then validation SHALL succeed with no errors

  @AC-23.2
  Scenario: Capability enforcement rejects unsupported features
    Given a profile requiring force-feedback on a non-FFB device
    When capability enforcement is applied
    Then the enforcement SHALL report a capability violation

  @AC-23.3
  Scenario: Profile canonicalization produces stable hashes
    Given two profiles with identical logical configuration but different field order
    When both are canonicalized
    Then their effective hashes SHALL be equal

  @AC-23.4
  Scenario: merge_with applies more-specific profile overrides
    Given a global profile and an aircraft-specific override profile
    When merge_with is called with the override as the more-specific source
    Then the resulting profile SHALL use override values for fields present in the override
    And retain global values for fields absent from the override
