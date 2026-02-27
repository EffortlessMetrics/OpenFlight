Feature: Thrustmaster HOTAS Warthog Long-Term Support
  As a flight simulation enthusiast
  I want the HOTAS Warthog to be Tier 1 supported indefinitely
  So that I can rely on first-class support for my hardware investment

  Background:
    Given the OpenFlight device manifest library is available

  Scenario: Warthog stick and throttle have complete axis and button mappings
    When the Warthog stick and throttle manifests are loaded
    Then all physical axes and buttons are mapped in the manifest with no gaps

  Scenario: Warthog manifests cover all known firmware revisions
    When the Warthog device manifests are inspected
    Then there is a manifest entry for each known firmware revision of the Warthog stick and throttle

  Scenario: Warthog integration tests run on every PR
    Given a CI pipeline configuration
    When a pull request is opened against the main branch
    Then the Warthog integration test suite is included in the required checks

  Scenario: Warthog profile template is included in the template library
    When the OpenFlight profile template library is listed
    Then at least one Warthog-specific profile template is present
