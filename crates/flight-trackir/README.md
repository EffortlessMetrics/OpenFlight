# flight-trackir

TrackIR / head tracking adapter for OpenFlight via the OpenTrack UDP bridge protocol.

## Overview

NaturalPoint TrackIR and compatible head-trackers (OpenTrack, FreePIE) can output
6DOF pose data over UDP as 48-byte packets (6 × f64 LE).  This crate parses those
packets and normalises the values into the `[-1.0, 1.0]` range used by the
OpenFlight axis engine.

## UDP packet layout

| Offset | Size | Field   | Unit |
|--------|------|---------|------|
| 0      | 8    | x       | mm   |
| 8      | 8    | y       | mm   |
| 16     | 8    | z       | mm   |
| 24     | 8    | yaw     | deg  |
| 32     | 8    | pitch   | deg  |
| 40     | 8    | roll    | deg  |

## Normalisation

| Axis    | Raw range | Normalised |
|---------|-----------|------------|
| yaw     | ±180°     | ±1.0       |
| pitch   | ±90°      | ±1.0       |
| roll    | ±180°     | ±1.0       |
| x, y, z | ±100 mm  | ±1.0 (clamped) |

## OpenTrack setup

In OpenTrack: *Output* → **UDP over network** → host `127.0.0.1`, port `4242`.
