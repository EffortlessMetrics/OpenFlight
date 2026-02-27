@REQ-67
Feature: Cloud profile cache coherence, client URL encoding, and extended sanitize

  @AC-67.1
  Scenario: Cache entry is not expired within its TTL window
    Given a cache entry with a TTL of 60 seconds
    When the entry is checked before its TTL expires
    Then is_expired SHALL return false

  @AC-67.1
  Scenario: Cache entry is expired after its TTL elapses
    Given a cache entry with a TTL of 0 seconds
    When the entry is checked after its TTL expires
    Then is_expired SHALL return true

  @AC-67.2
  Scenario: Evicting a cache entry removes it
    Given a cache with one entry
    When evict is called for that entry's key
    Then the entry SHALL no longer be present in the cache

  @AC-67.2
  Scenario: Listing cached entries only returns fresh entries
    Given a cache with one fresh and one expired entry
    When list_cached is called
    Then only the fresh entry SHALL be returned

  @AC-67.3
  Scenario: URL encoding passes through safe characters unchanged
    Given a URL encoding function
    When safe alphanumeric characters are encoded
    Then the output SHALL be identical to the input

  @AC-67.3
  Scenario: URL encoding percent-encodes spaces and slashes
    Given a URL encoding function
    When a string containing spaces and slashes is encoded
    Then spaces SHALL become %20 and slashes SHALL become %2F

  @AC-67.4
  Scenario: CloudClient constructs successfully with default config
    Given a default CloudClient configuration
    When a CloudClient is constructed
    Then the client SHALL be created without error and SHALL have expected base_url and timeout fields

  @AC-67.5
  Scenario: Sanitize for upload preserves profile data
    Given a cloud profile with sim name, schema version, and axes
    When sanitize_for_upload is called
    Then the original profile SHALL not be modified and the sanitized copy SHALL have lowercased sim and normalized schema

  @AC-67.6
  Scenario: Valid profile is accepted for publish
    Given a profile with a valid title and axes
    When validate_for_publish is called
    Then the profile SHALL be accepted without error

  @AC-67.6
  Scenario: Profile with empty title is rejected for publish
    Given a profile with an empty title
    When validate_for_publish is called
    Then the validation SHALL return an error

  @AC-67.7
  Scenario: ProfileListing score calculations are correct
    Given a ProfileListing with positive and negative votes
    When the score is calculated
    Then the score SHALL equal upvotes minus downvotes

  @AC-67.7
  Scenario: ProfileListing JSON round-trips preserve all fields
    Given a ProfileListing with all fields set
    When it is serialized to JSON and deserialized
    Then all fields SHALL be preserved in the round-trip

  @AC-67.8
  Scenario: SortOrder display formatting is correct
    Given the set of SortOrder variants
    When each variant is formatted for display
    Then the output SHALL match the expected label strings

  @AC-67.8
  Scenario: VoteResult computes correct net score
    Given a VoteResult with upvotes and downvotes
    When the net score is computed
    Then it SHALL equal upvotes minus downvotes
