Feature: Profile Import from URL
  As a flight simulation enthusiast
  I want to import a profile from a URL
  So that I can easily share and distribute profiles

  Background:
    Given the OpenFlight service is running

  Scenario: Importing a profile from an HTTPS URL
    Given a valid profile is hosted at an HTTPS URL
    When I run "flightctl profile import <url>" with that URL
    Then the profile is downloaded and validated successfully

  Scenario: HTTP and HTTPS URLs are both supported
    Given a valid profile is hosted at an HTTP URL
    When I run "flightctl profile import <url>" with that URL
    Then the profile is downloaded and validated successfully

  Scenario: Profile checksum is verified after download
    Given a valid profile with a known checksum is hosted at a URL
    When I run "flightctl profile import <url>"
    Then the downloaded profile checksum is verified before it is applied

  Scenario: Import fails gracefully on invalid URL
    Given an invalid or unreachable URL
    When I run "flightctl profile import <url>" with that URL
    Then the command exits with a non-zero code and a clear error message describing the failure
