---
doc_id: REF-KSP-MAPPING
kind: reference
area: ksp
status: current
links:
  - REQ-41
---

# KSP kRPC → BusSnapshot Mapping

The `flight-ksp` crate connects to Kerbal Space Program via the [kRPC mod](https://krpc.github.io/krpc/) over a local TCP socket and maps vessel telemetry onto the Flight Hub `BusSnapshot`.

## Prerequisites

1. Install kRPC in KSP: place `KRPC.dll` and `KRPC.SpaceCenter.dll` in `GameData/`.
2. Start KSP, open a save, and enable the kRPC server (default port 50000).
3. Ensure the vessel is loaded and the simulation is running.

## Connection parameters

| Config field | Default | Notes |
|---|---|---|
| `krpc_host` | `"127.0.0.1"` | kRPC server IP |
| `krpc_port` | `50000` | kRPC RPC port (not stream port) |
| `poll_rate_hz` | `20.0` | Telemetry update rate |
| `connection_timeout` | `5 s` | Per-connect attempt timeout |
| `reconnect_delay` | `2 s` | Delay between reconnection attempts |

## kRPC procedure → BusSnapshot field

| kRPC Procedure | kRPC Unit | BusSnapshot Field | Conversion |
|---|---|---|---|
| `SpaceCenter.Vessel_get_Name` | string | `aircraft` | vessel name as AircraftId |
| `SpaceCenter.Vessel_get_Situation` | enum i32 | `validity.*` | see Situation Values |
| `SpaceCenter.Vessel_get_Latitude` | degrees f64 | `navigation.latitude` | direct |
| `SpaceCenter.Vessel_get_Longitude` | degrees f64 | `navigation.longitude` | direct |
| `SpaceCenter.Vessel_get_MeanAltitude` | metres f64 | `environment.altitude` | × 3.280840 → feet |
| `SpaceCenter.Flight_get_Pitch` | degrees f32 | `kinematics.pitch` | direct |
| `SpaceCenter.Flight_get_Roll` | degrees f32 | `kinematics.bank` | direct |
| `SpaceCenter.Flight_get_Heading` | degrees 0–360 f32 | `kinematics.heading` | normalize to −180…+180¹ |
| `SpaceCenter.Flight_get_Speed` | m/s f64 | `kinematics.tas` | × 1.943844 → knots |
| `SpaceCenter.Flight_get_EquivalentAirSpeed` | m/s f64 | `kinematics.ias` | × 1.943844 → knots |
| `SpaceCenter.Flight_get_VerticalSpeed` | m/s f64 | `kinematics.vertical_speed` | × 196.850394 → fpm |
| `SpaceCenter.Flight_get_GForce` | g f64 | `kinematics.g_force` | clamped to −20…+20 g |

¹ Heading normalization: `if heading > 180.0 { heading - 360.0 } else { heading }`

## KSP Situation Values

The `VesselSituation` enum reported by `Vessel_get_Situation`:

| Value | Constant | `safe_for_ffb` | `attitude_valid` | Notes |
|---|---|---|---|---|
| 0 | `LANDED` | false | false | On the ground |
| 1 | `SPLASHED` | false | false | On water surface |
| 2 | `PRELAUNCH` | false | false | On launchpad |
| 3 | `FLYING` | **true** | true | In atmosphere |
| 4 | `SUB_ORBITAL` | false | true | Above atmo but < orbital |
| 5 | `ORBITING` | false | true | Stable orbit |
| 6 | `ESCAPING` | false | true | Escape trajectory |
| 7 | `DOCKED` | false | false | Docked to another vessel |

## Not yet mapped

The following data are available via kRPC but not yet fetched:

- Angular rates (pitch/roll/yaw rate): `Flight_get_PitchRate`, `Flight_get_RollRate`, `Flight_get_YawRate`
- Angle of attack / sideslip: `Flight_get_AngleOfAttack`, `Flight_get_SideslipAngle`
- Engine thrust/fuel data
- Ground speed: `Flight_get_HorizontalSpeed`

## Wire protocol

kRPC uses length-delimited protobuf over plain TCP (not gRPC/HTTP2). Each message is prefixed with a protobuf varint containing the message byte count. The `flight-ksp` crate handles this framing in `connection::KrpcConnection`.

Object handles (vessels, reference frames, flight objects) are `u64` values encoded as protobuf `uint64` in field 1. Scalar values (f32, f64, bool, string, i32) are similarly encoded with field 1.
