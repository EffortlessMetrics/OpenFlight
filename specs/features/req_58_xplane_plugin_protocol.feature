# SPDX-License-Identifier: MIT OR Apache-2.0
# Requirement: REQ-58 — X-Plane Plugin Protocol
# Acceptance criteria: AC-58.1 through AC-58.5

@REQ-58 @xplane-plugin @protocol
Feature: X-Plane plugin newline-delimited JSON protocol
  As Flight Hub
  I want a stable newline-delimited JSON protocol between Flight Hub and the X-Plane plugin
  So that messages can be reliably serialised, transmitted, and deserialised without data loss

  # ── AC-58.1: PluginMessage JSON round-trip ───────────────────────────────────

  @AC-58.1
  Scenario: Handshake PluginMessage survives a JSON round-trip
    Given a PluginMessage::Handshake with version "1.0" and capabilities ["subscribe"]
    When the message is serialised to JSON
    And deserialised back to a PluginMessage
    Then the resulting variant SHALL be Handshake
    And the version field SHALL equal "1.0"

  # ── AC-58.2: PluginResponse JSON round-trip ──────────────────────────────────

  @AC-58.2
  Scenario: Pong PluginResponse survives a JSON round-trip
    Given a PluginResponse::Pong with id 42 and timestamp 999
    When the response is serialised to JSON
    And deserialised back to a PluginResponse
    Then the resulting variant SHALL be Pong
    And the id field SHALL equal 42
    And the timestamp field SHALL equal 999

  @AC-58.2
  Scenario: Error PluginResponse with all-None optional fields survives a JSON round-trip
    Given a PluginResponse::Error with id None, error "not found", and details None
    When the response is serialised to JSON
    And deserialised back to a PluginResponse
    Then the resulting variant SHALL be Error
    And the id field SHALL be None
    And the error field SHALL equal "not found"
    And the details field SHALL be None

  # ── AC-58.3: Malformed JSON returns an error ──────────────────────────────────

  @AC-58.3
  Scenario: Completely malformed JSON returns an error
    Given the input string "{not valid json}"
    When it is deserialised as a PluginMessage
    Then the result SHALL be Err
    And the process SHALL NOT panic

  @AC-58.3
  Scenario: Valid JSON with an unknown type tag returns an error
    Given the input string {"type":"UnknownVariant"}
    When it is deserialised as a PluginMessage
    Then the result SHALL be Err
    And the process SHALL NOT panic

  # ── AC-58.4: Handshake contains version and capabilities ─────────────────────

  @AC-58.4
  Scenario: Serialised Handshake JSON contains version and capabilities keys
    Given a PluginMessage::Handshake with version "2.0" and capabilities ["subscribe","commands"]
    When the message is serialised to JSON
    Then the JSON string SHALL contain the key "version"
    And the JSON string SHALL contain the key "capabilities"

  # ── AC-58.5: GetDataRef and DataRefValue carry matching IDs ──────────────────

  @AC-58.5
  Scenario: GetDataRef serialises with the correct id
    Given a PluginMessage::GetDataRef with id 7 and name "sim/cockpit/autopilot/altitude"
    When the message is serialised to JSON
    Then the JSON string SHALL contain "GetDataRef"
    And the JSON string SHALL contain "sim/cockpit/autopilot/altitude"

  @AC-58.5
  Scenario: DataRefValue round-trip preserves the id matching the originating GetDataRef
    Given a PluginResponse::DataRefValue with id 99, name "sim/test/dataref", value 1.5, timestamp 1000
    When the response is serialised to JSON
    And deserialised back to a PluginResponse
    Then the resulting variant SHALL be DataRefValue
    And the id field SHALL equal 99
    And the name field SHALL equal "sim/test/dataref"
