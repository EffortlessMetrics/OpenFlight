@REQ-421 @infra
Feature: Profile Compile Cache — Cache Compiled Profiles to Avoid Re-Compilation

  @AC-421.1
  Scenario: Compiled profile is cached with a hash of the source profile
    Given a profile that has been compiled
    When the cache entry is inspected
    Then it SHALL be keyed by a content hash of the source profile

  @AC-421.2
  Scenario: Cache hit returns the pre-compiled profile without re-parsing
    Given a previously compiled and cached profile
    When the same source profile is requested again
    Then the pre-compiled profile SHALL be returned without triggering a parse+compile cycle

  @AC-421.3
  Scenario: Cache miss triggers a full parse and compile cycle
    Given a profile not present in the cache
    When it is requested
    Then a full parse+compile cycle SHALL be triggered and the result stored in cache

  @AC-421.4
  Scenario: Cache is invalidated when the source profile file changes
    Given a cached compiled profile
    When the source profile file is modified on disk
    Then the cache entry SHALL be invalidated on next access

  @AC-421.5
  Scenario: Cache size is bounded to a maximum of 5 profiles
    Given a cache with 5 profiles already stored
    When a sixth distinct profile is compiled
    Then the oldest or least-recently-used entry SHALL be evicted to maintain the limit

  @AC-421.6
  Scenario: Cache metrics (hits, misses, evictions) are exposed via the metrics endpoint
    Given the service metrics endpoint
    When the cache has been used
    Then profile_cache_hits, profile_cache_misses, and profile_cache_evictions metrics SHALL be present
