Feature: X-Plane Dataref Caching
  As a flight simulation enthusiast
  I want x-plane dataref caching
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Local dataref cache stores values with configurable time-to-live
    Given the system is configured for x-plane dataref caching
    When the feature is exercised
    Then local dataref cache stores values with configurable time-to-live

  Scenario: Cache invalidation occurs on dataref write or TTL expiry
    Given the system is configured for x-plane dataref caching
    When the feature is exercised
    Then cache invalidation occurs on dataref write or TTL expiry

  Scenario: Cache hit rate is tracked and reported via metrics endpoint
    Given the system is configured for x-plane dataref caching
    When the feature is exercised
    Then cache hit rate is tracked and reported via metrics endpoint

  Scenario: Cache size is bounded to prevent unbounded memory growth
    Given the system is configured for x-plane dataref caching
    When the feature is exercised
    Then cache size is bounded to prevent unbounded memory growth