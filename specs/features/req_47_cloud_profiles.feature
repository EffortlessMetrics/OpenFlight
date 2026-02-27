# REQ-47 Cloud Profile Repository
# Covers: community browse, download, publish, vote, cache, and validation

Feature: Cloud Profile Repository

  Background:
    Given the Flight Hub cloud profile client is initialised with a stub server

  # ── Listing ──────────────────────────────────────────────────────────────

  @AC-47.1
  Scenario: List profiles returns paginated results
    Given the stub server has 30 published profiles
    When I request page 1 with 25 profiles per page
    Then the response contains 25 profiles
    And the page metadata shows total=30, page=1, total_pages=2

  @AC-47.1
  Scenario: List profiles filtered by simulator
    Given the stub server has profiles for "msfs", "xplane", and "dcs"
    When I list profiles with sim="msfs"
    Then all returned profiles have sim="msfs"

  @AC-47.2
  Scenario: List profiles filtered by aircraft ICAO
    Given the stub server has profiles for aircraft "C172" and "B738"
    When I list profiles with aircraft_icao="C172"
    Then all returned profiles have aircraft_icao="C172"

  @AC-47.2
  Scenario: List defaults to top-rated sort order
    Given the ListFilter is constructed with defaults
    Then sort order is "top_rated"
    And page is 1
    And per_page is 25

  @AC-47.3
  Scenario: Iterate all pages via list()
    Given the stub server has 60 profiles
    When I call list() with per_page=25
    Then all 60 profiles are returned across 3 pages

  # ── Get / Cache ───────────────────────────────────────────────────────────

  @AC-47.3
  Scenario: Get a profile by ID fetches from server
    Given a profile with id="abc123" exists on the server
    When I call get("abc123")
    Then the returned profile has id="abc123"
    And the profile is stored in the local cache

  @AC-47.4
  Scenario: Subsequent get returns from cache
    Given a profile with id="abc123" is in the local cache and not expired
    When I call get("abc123")
    Then no network request is made
    And the cached profile is returned

  @AC-47.5
  Scenario: Expired cache entry triggers a fresh network fetch
    Given a profile with id="abc123" is in the local cache but expired
    When I call get("abc123")
    Then a network request is made to fetch the profile
    And the cache is refreshed with the new data

  # ── Publish ───────────────────────────────────────────────────────────────

  @AC-47.6
  Scenario: Publish a valid sanitized profile
    Given a local profile with sim="MSFS" and valid axes
    When I publish it with title="My Setup" and description="Test"
    Then sanitize_for_upload normalises sim to "msfs"
    And the server receives the sanitised profile
    And the published listing contains the assigned id

  @AC-47.6
  Scenario: Publish rejected when title is too short
    Given a local profile with valid axes
    When I attempt to publish it with title="ab"
    Then validate_for_publish returns an error containing "too short"

  @AC-47.7
  Scenario: Publish rejected when title is too long
    Given a local profile with valid axes
    When I attempt to publish it with a 90-character title
    Then validate_for_publish returns an error containing "too long"

  # ── Vote ─────────────────────────────────────────────────────────────────

  @AC-47.7
  Scenario: Upvote a profile
    Given a profile "abc123" with score=5
    When I vote "up" on profile "abc123"
    Then the VoteResult shows upvotes incremented by 1

  @AC-47.8
  Scenario: Downvote a profile
    Given a profile "abc123" with score=5
    When I vote "down" on profile "abc123"
    Then the VoteResult shows downvotes incremented by 1

  @AC-47.8
  Scenario: Remove a vote from a profile
    Given I have previously voted on profile "abc123"
    When I call remove_vote("abc123")
    Then the server acknowledges with HTTP 204

  # ── Cache management ──────────────────────────────────────────────────────

  @AC-47.9
  Scenario: Clear cache removes all stored profiles
    Given the local cache contains profiles "abc123" and "def456"
    When I call cache.clear()
    Then the cache directory contains no profile files
    And the cache index is empty

  @AC-47.9
  Scenario: Evict a single profile from cache
    Given the local cache contains profiles "abc123" and "def456"
    When I evict "abc123"
    Then "abc123" is not in the cache
    And "def456" is still in the cache
