@REQ-106 @product
Feature: Core type safety for flight-core fundamental types

  @AC-106.1
  Scenario: SimId Display format is non-empty and printable for every variant
    Given any SimId variant (MSFS, X-Plane, DCS, KSP, Unknown, etc.)
    When the variant is formatted with Display
    Then the resulting string SHALL be non-empty and contain only printable Unicode characters

  @AC-106.2
  Scenario: DeviceId ordering is stable and reflexive
    Given two DeviceId values constructed with the same vendor and product IDs
    When compared with the Ord trait
    Then equal DeviceIds SHALL compare as equal on every call

  @AC-106.3
  Scenario: AircraftId ICAO value survives JSON serialization round-trip unchanged
    Given a Profile whose AircraftId ICAO field is "C172"
    When the profile is serialized to JSON and deserialized back
    Then the ICAO field SHALL equal "C172" without modification

  @AC-106.4
  Scenario: FlightError variants produce non-empty display messages containing context
    Given a FlightError::Configuration("bad_key") value
    When the error is formatted with Display
    Then the message SHALL contain "Configuration error" and "bad_key"

  @AC-106.5
  Scenario: Random byte sequences fed to the profile JSON parser never panic
    Given an arbitrary byte sequence that is not a valid profile JSON document
    When the byte sequence is passed to serde_json::from_str::<Profile>
    Then the call SHALL return Err and SHALL NOT panic
