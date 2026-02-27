@REQ-518 @product
Feature: Profile Merge From Base

  @AC-518.1 @AC-518.2
  Scenario: Derived profile overrides base profile fields
    Given a base profile defines a deadzone of 5% for the pitch axis
    And a derived profile references the base and sets deadzone to 10%
    When the profiles are merged
    Then the resulting deadzone for the pitch axis SHALL be 10%

  @AC-518.3
  Scenario: Merge chain supports up to 3 levels of inheritance
    Given a profile chain of global base, simulator override, and aircraft override
    When the three-level merge is performed
    Then the aircraft override values SHALL take final precedence

  @AC-518.4
  Scenario: Circular base references are detected and rejected
    Given two profiles each referencing the other as their base
    When the profile loader attempts to resolve the chain
    Then an error SHALL be returned identifying the circular reference
