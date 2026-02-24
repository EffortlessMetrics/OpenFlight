@motion @platform @6dof
Feature: REQ-45 Motion Platform 6DOF Support
  As a flight simulator enthusiast with a motion platform,
  I want OpenFlight to drive my 6DOF motion platform via the flight-motion crate,
  So that I get realistic motion cues synchronized with in-sim flight dynamics.

  Background:
    Given the flight-motion crate is built with default configuration
    And the sample rate is 60 Hz (dt = 1/60 s)

  # ── AC-45.1 MotionFrame normalization ──────────────────────────────────────

  @unit @normalization
  Scenario Outline: Motion frame channels are normalized to -1.0..1.0
    Given a motion frame with <channel> = <raw_value>
    When the frame is clamped
    Then <channel> is <expected>

    Examples:
      | channel | raw_value | expected |
      | surge   | 2.0       | 1.0      |
      | sway    | -5.0      | -1.0     |
      | heave   | 0.5       | 0.5      |
      | roll    | 0.0       | 0.0      |
      | pitch   | -1.0      | -1.0     |
      | yaw     | 3.0       | 1.0      |

  # ── AC-45.2 SimTools UDP format ─────────────────────────────────────────────

  @unit @simtools
  Scenario: SimTools string format is correct
    Given a motion frame with surge=0.5, sway=-0.25, heave=1.0, roll=0.0, pitch=0.0, yaw=-0.5
    When the SimTools string is generated
    Then the output is "A50B-25C100D0E0F-50\n"

  @unit @simtools
  Scenario: Neutral frame produces all-zero SimTools string
    Given a neutral motion frame (all channels = 0.0)
    When the SimTools string is generated
    Then the output is "A0B0C0D0E0F0\n"

  # ── AC-45.3 Washout filter — translational channels ─────────────────────────

  @unit @washout
  Scenario: High-pass filter passes acceleration onset cue
    Given a high-pass filter with corner frequency 0.5 Hz and sample dt 1/250 s
    When a step input of 1.0 is applied on the first tick
    Then the output is greater than 0.9

  @unit @washout
  Scenario: High-pass filter washes out sustained acceleration
    Given a high-pass filter with corner frequency 0.5 Hz and sample dt 1/60 s
    When 600 ticks of constant input 1.0 are applied
    Then the output is less than 0.02 in absolute value

  @unit @washout
  Scenario: Low-pass filter converges to steady-state input
    Given a low-pass filter with corner frequency 5.0 Hz and sample dt 1/250 s
    When 5000 ticks of constant input 1.0 are applied
    Then the output is approximately 1.0 (within 0.01)

  # ── AC-45.4 MotionMapper — BusSnapshot mapping ─────────────────────────────

  @integration @mapper
  Scenario: Neutral BusSnapshot washes out all channels
    Given a MotionMapper with default configuration at 60 Hz
    When 2000 ticks of a neutral BusSnapshot are processed
    Then all motion frame channels are within 0.01 of zero

  @integration @mapper
  Scenario: Zero intensity produces neutral output
    Given a MotionMapper with intensity = 0.0
    When any BusSnapshot is processed
    Then the output is a neutral motion frame

  @integration @mapper
  Scenario: Disabled channel always outputs zero
    Given a MotionMapper with the roll channel disabled
    When any BusSnapshot (including high bank angle) is processed
    Then the roll channel is exactly 0.0

  # ── AC-45.5 SimTools UDP output ─────────────────────────────────────────────

  @integration @udp
  Scenario: SimToolsUdpOutput sends correctly formatted datagrams
    Given a local UDP listener on an ephemeral port
    And a SimToolsUdpOutput connected to that port
    When a frame with surge=0.5, sway=-0.25, heave=1.0, roll=0, pitch=0, yaw=-0.5 is sent
    Then the received datagram is "A50B-25C100D0E0F-50\n"

  # ── AC-45.6 Configuration ───────────────────────────────────────────────────

  @unit @config
  Scenario: Default MotionConfig has safe production values
    When the default MotionConfig is created
    Then intensity is 0.8
    And max_g is 3.0
    And max_angle_deg is 30.0
    And all channels are enabled with gain 1.0

  @unit @config
  Scenario: Per-channel gain scales output proportionally
    Given a MotionMapper with roll gain = 2.0 and intensity = 1.0
    When a sustained 15-degree bank angle is applied (half of max_angle_deg=30)
    Then the roll output converges to approximately 1.0 (clamped)

  @unit @config
  Scenario: Inverted channel flips sign
    Given a MotionMapper with the heave channel inverted
    When a positive g-force is applied
    Then the heave output is negative
