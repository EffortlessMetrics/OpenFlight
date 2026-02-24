# SPDX-License-Identifier: MIT OR Apache-2.0
# Requirement: REQ-42 — X-Plane 11/12 Integration
# Acceptance criteria: AC-42.1 through AC-42.5

@xplane @adapter
Feature: X-Plane 11/12 adapter
  As Flight Hub
  I want to receive normalized telemetry from X-Plane via UDP DataRef output
  So that downstream consumers have a consistent BusSnapshot regardless of sim

  Background:
    Given an X-Plane adapter with default configuration
    And X-Plane is streaming UDP DataRef packets to port 49000

  # ── AC-42.1: Basic telemetry ingestion ──────────────────────────────────────

  Scenario: IAS, TAS and ground speed are ingested and converted from m/s to knots
    Given X-Plane reports indicated_airspeed = 51.44 m/s
    And X-Plane reports true_airspeed = 54.0 m/s
    And X-Plane reports groundspeed = 52.0 m/s
    When the adapter processes the DataRef packet
    Then BusSnapshot.kinematics.ias SHALL be approximately 100.0 knots
    And BusSnapshot.kinematics.tas SHALL be approximately 104.9 knots
    And BusSnapshot.kinematics.ground_speed SHALL be approximately 101.1 knots

  Scenario: Attitude angles are ingested and converted from degrees to radians
    Given X-Plane reports theta = 5.0 degrees
    And X-Plane reports phi = 15.0 degrees
    And X-Plane reports psi = 270.0 degrees
    When the adapter processes the DataRef packet
    Then BusSnapshot.kinematics.pitch SHALL be approximately 0.0873 radians
    And BusSnapshot.kinematics.bank SHALL be approximately 0.2618 radians
    And BusSnapshot.kinematics.heading SHALL be approximately -1.5708 radians

  Scenario: Heading psi > 180 is normalized to the range (-180, +180]
    Given X-Plane reports psi = 200.0 degrees
    When the adapter processes the DataRef packet
    Then BusSnapshot.kinematics.heading SHALL be approximately -2.792 radians

  # ── AC-42.2: Angular rate mapping ───────────────────────────────────────────

  Scenario: Angular rates P, Q, R are converted from deg/s to rad/s
    Given X-Plane reports body-axis roll rate P = 57.296 deg/s
    And X-Plane reports body-axis pitch rate Q = 28.648 deg/s
    And X-Plane reports body-axis yaw rate R = 11.459 deg/s
    When the adapter processes the DataRef packet
    Then BusSnapshot.angular_rates.p SHALL be approximately 1.0 rad/s
    And BusSnapshot.angular_rates.q SHALL be approximately 0.5 rad/s
    And BusSnapshot.angular_rates.r SHALL be approximately 0.2 rad/s

  # ── AC-42.3: Engine telemetry ────────────────────────────────────────────────

  Scenario: Engine N1, EGT, oil pressure and fuel flow are populated when DataRefs are present
    Given X-Plane reports ENGN_running[0] = 1
    And X-Plane reports ENGN_N1_[0] = 85.0 percent
    And X-Plane reports ENGN_EGT[0] = 620.0 Celsius
    And X-Plane reports ENGN_FF_[0] = 0.05 kg/s
    And X-Plane reports ENGN_oilp[0] = 55.0 PSI
    When the adapter processes the DataRef packet
    Then BusSnapshot.engines[0].running SHALL be true
    And BusSnapshot.engines[0].rpm SHALL be approximately 85.0 percent
    And BusSnapshot.engines[0].egt SHALL be approximately 620.0 Celsius
    And BusSnapshot.engines[0].oil_pressure SHALL be approximately 55.0 PSI
    And BusSnapshot.engines[0].fuel_flow SHALL be approximately 59.2 gal/hr

  # ── AC-42.4: Pressure altitude ───────────────────────────────────────────────

  Scenario: Pressure altitude is populated from the cockpit altimeter DataRef
    Given X-Plane reports altitude_ft_pilot = 10000.0 feet
    When the adapter processes the DataRef packet
    Then BusSnapshot.environment.pressure_altitude SHALL be approximately 10000.0 feet

  # ── AC-42.5: Plugin interface TCP I/O ────────────────────────────────────────

  Scenario: PluginInterface sends a newline-terminated JSON handshake on connection
    Given a Flight Hub plugin interface listening on TCP port 52000
    When an X-Plane plugin connects
    Then Flight Hub SHALL write a JSON {"type":"Handshake",...} line to the socket
    And the plugin SHALL reply with {"type":"HandshakeAck","status":"ready",...}
    And PluginInterface.is_connected() SHALL return true

  Scenario: PluginInterface reads a DataRefValue response from the plugin
    Given a connected PluginInterface
    When the plugin writes {"type":"DataRefValue","id":1,"name":"sim/test","value":{"Float":42.0},"timestamp":0}\n
    Then the pending request for id=1 SHALL resolve with DataRefValue::Float(42.0)
