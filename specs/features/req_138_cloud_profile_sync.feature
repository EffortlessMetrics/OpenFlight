@REQ-138 @product
Feature: Cloud profile sync  @AC-138.1
  Scenario: Upload serializes profile and metadata
    Given a valid local profile with display name "My Profile" and version 3
    When the profile is prepared for cloud upload
    Then the serialized payload SHALL contain the profile data and the metadata including the display name and version  @AC-138.2
  Scenario: Download deserializes profile correctly
    Given a cloud API response containing a valid serialized profile payload
    When the payload is deserialized
    Then the resulting profile SHALL equal the original profile that was uploaded  @AC-138.3
  Scenario: Conflict local-wins strategy preserves local changes
    Given a local profile modified at T+10 and a cloud profile modified at T+5
    When the local-wins conflict resolution strategy is applied
    Then the resolved profile SHALL match the local version  @AC-138.4
  Scenario: Pagination fetches all pages for large list
    Given a cloud profile list spanning 3 pages of 20 items each
    When a full list fetch is performed with pagination
    Then all 60 profiles SHALL be returned in the final result  @AC-138.5
  Scenario: Network error during upload returns typed error
    Given a cloud sync client configured to simulate a network timeout
    When an upload is attempted
    Then the result SHALL be a typed NetworkError variant  @AC-138.6
  Scenario: Schema mismatch on download returns error
    Given a cloud API response containing a profile serialized with an incompatible schema version
    When the payload is deserialized
    Then the result SHALL be a SchemaMismatch error  @AC-138.7
  Scenario: Sanitize-for-upload is idempotent
    Given a profile that has been sanitized for upload once
    When sanitize-for-upload is applied a second time
    Then the resulting payload SHALL be identical to the first sanitized payload  @AC-138.8
  Scenario: Vote direction serializes as expected
    Given a vote direction of Up
    When the vote direction is serialized to its wire representation
    Then the serialized value SHALL be the canonical Up representation
    And a vote direction of Down SHALL serialize to the canonical Down representation
