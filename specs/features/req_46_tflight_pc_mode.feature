@hotas @thrustmaster @pc-mode @detents
Feature: REQ-46 T.Flight HOTAS PC Mode Detection and Throttle Detents
  As an OpenFlight user with a Thrustmaster T.Flight HOTAS 4,
  I want the system to detect whether my device is in PC mode,
  And I want throttle detent crossings to be tracked in software,
  So that I get actionable guidance when in console mode and tactile-feedback events when crossing detent gates.

  # ── Background ────────────────────────────────────────────────────────────

  Background:
    Given the flight-hotas-thrustmaster crate is available

  # ── AC-17.1 PC Mode Detection — report length ──────────────────────────────

  @unit @pc-mode
  Scenario Outline: PC mode classified by report length
    Given a PcModeDetector with confirm_count = 1
    When a report of <length> bytes is processed
    Then the status SHALL be <expected_status>

    Examples:
      | length | expected_status |
      | 8      | PcMode          |
      | 9      | PcMode          |
      | 12     | PcMode          |
      | 5      | ConsoleMode     |
      | 0      | ConsoleMode     |

  # ── AC-17.2 PC Mode Detection — confirmation hysteresis ────────────────────

  @unit @pc-mode
  @AC-46.1
  Scenario: Detector requires confirm_count consecutive reports before committing
    Given a PcModeDetector with confirm_count = 3
    When two 8-byte reports are processed
    Then the status SHALL be Unknown
    When a third 8-byte report is processed
    Then the status SHALL be PcMode

  @unit @pc-mode
  @AC-46.2
  Scenario: A different-length report resets the run counter
    Given a PcModeDetector with confirm_count = 3
    When two 8-byte reports are processed
    And one 5-byte report is processed
    Then the status SHALL still be Unknown

  # ── AC-17.3 PC Mode Detection — console mode guidance ──────────────────────

  @unit @pc-mode
  @AC-46.3
  Scenario: Console mode guidance is returned when in console mode
    Given a PcModeDetector with confirm_count = 1
    When a 5-byte report is processed
    Then console_mode_guidance SHALL return a non-empty string
    And the guidance SHALL mention "Share" and "throttle"

  @unit @pc-mode
  @AC-46.3
  Scenario: No guidance returned when in PC mode
    Given a PcModeDetector with confirm_count = 1
    When an 8-byte report is processed
    Then console_mode_guidance SHALL return None

  # ── AC-17.4 PC Mode Detection — reset ──────────────────────────────────────

  @unit @pc-mode
  @AC-46.4
  Scenario: Reset clears committed status back to Unknown
    Given a PcModeDetector with confirm_count = 1
    When an 8-byte report is processed
    Then the status SHALL be PcMode
    When the detector is reset
    Then the status SHALL be Unknown

  # ── AC-17.5 Throttle Detent — Entered event on zone entry ──────────────────

  @unit @detents
  @AC-46.5
  Scenario: Entered event fires when throttle moves into detent zone
    Given a ThrottleDetentTracker with the default HOTAS 4 idle detent
    And the throttle is currently at 0.20 (outside zone)
    When the throttle moves to 0.05 (inside zone)
    Then an Entered event SHALL be emitted with detent_index = 0

  # ── AC-17.6 Throttle Detent — Exited event on zone exit ───────────────────

  @unit @detents
  @AC-46.6
  Scenario: Exited event fires when throttle leaves detent zone
    Given a ThrottleDetentTracker with the default HOTAS 4 idle detent
    And the throttle has entered the detent zone at 0.05
    When the throttle moves to 0.00 (below zone)
    Then an Exited event SHALL be emitted with detent_index = 0

  # ── AC-17.7 Throttle Detent — hysteresis prevents chatter ─────────────────

  @unit @detents
  @AC-46.7
  Scenario: No duplicate Entered events while throttle stays inside zone
    Given a ThrottleDetentTracker with the default HOTAS 4 idle detent
    And the throttle has entered the detent zone
    When the throttle is updated at 0.04, 0.05, 0.06 (all inside zone)
    Then no further events SHALL be emitted

  # ── AC-17.8 Throttle Detent — Entered fires again after exit/re-enter ──────

  @unit @detents
  @AC-46.8
  Scenario: Entered fires again after a complete exit and re-entry cycle
    Given a ThrottleDetentTracker with the default HOTAS 4 idle detent
    When the throttle enters the zone at 0.05
    And the throttle exits to 0.20
    And the throttle re-enters at 0.05
    Then a second Entered event SHALL be emitted

  # ── AC-17.9 Throttle Detent — reset ────────────────────────────────────────

  @unit @detents
  @AC-46.9
  Scenario: Reset allows Entered to fire again from inside state
    Given a ThrottleDetentTracker with the default HOTAS 4 idle detent
    And the throttle is inside the detent zone
    When the tracker is reset
    And the throttle is updated at 0.05 again
    Then an Entered event SHALL be emitted

  # ── AC-17.10 Throttle Detent — default HOTAS 4 config ─────────────────────

  @unit @detents
  @AC-46.9
  Scenario: Default HOTAS 4 detent is at 5% with ±2% half-width
    When ThrottleDetentConfig::hotas4_idle() is created
    Then position SHALL be 0.05
    And half_width SHALL be 0.02
    And the zone bounds SHALL be [0.03, 0.07]

  # ── AC-17.11 Fixture integrity ─────────────────────────────────────────────

  @integration @fixtures
  @AC-46.7
  Scenario: Synthetic merged fixture parses correctly
    Given the receipt fixture "merged_centered.bin"
    When parsed as a merged-mode report
    Then roll and pitch SHALL be approximately 0.0
    And throttle SHALL be approximately 0.5
    And buttons SHALL equal 0

  @integration @fixtures
  @AC-46.8
  Scenario: Synthetic separate fixture parses correctly
    Given the receipt fixture "separate_centered.bin"
    When parsed as a separate-mode report
    Then rocker SHALL be present
    And buttons SHALL equal 0
    And hat SHALL equal 0

  @integration @fixtures
  @AC-46.9
  Scenario: Console mode fixture triggers PC mode warning
    Given the receipt fixture "console_mode.bin" (5 bytes)
    And a PcModeDetector with confirm_count = 1
    When the fixture bytes are fed to the detector
    Then the status SHALL be ConsoleMode
    And console_mode_guidance SHALL be non-empty
