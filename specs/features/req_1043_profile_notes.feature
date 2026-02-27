@REQ-1043
Feature: Profile Notes
  @AC-1043.1
  Scenario: Users can attach text notes to any profile
    Given the system is configured for REQ-1043
    When the feature condition is met
    Then users can attach text notes to any profile

  @AC-1043.2
  Scenario: Notes are stored within the profile file format
    Given the system is configured for REQ-1043
    When the feature condition is met
    Then notes are stored within the profile file format

  @AC-1043.3
  Scenario: Notes are displayed in profile listing and detail views
    Given the system is configured for REQ-1043
    When the feature condition is met
    Then notes are displayed in profile listing and detail views

  @AC-1043.4
  Scenario: Notes are preserved during profile export and import
    Given the system is configured for REQ-1043
    When the feature condition is met
    Then notes are preserved during profile export and import
