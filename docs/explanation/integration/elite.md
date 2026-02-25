# Elite: Dangerous Integration

This document details exactly what Flight Hub reads from Elite: Dangerous, including files accessed, field mappings, and how to set up the integration.

## Overview

Flight Hub integrates with Elite: Dangerous through two file-based APIs that the game writes continuously:

| Data Source | Update Rate | Content |
|-------------|-------------|---------|
| `Status.json` | ~4 Hz | Ship flags, pips, fuel, cargo, GUI focus |
| `Journal.<date>.<session>.log` | Event-driven | Typed game events (FSD jumps, docking, etc.) |

Flight Hub **reads** these files; it does not modify any game files or inject code into the process.

## Files Accessed

### `Status.json`

**Location (Windows):**
```
%USERPROFILE%\Saved Games\Frontier Developments\Elite Dangerous\Status.json
```

**Purpose:** Polled by Flight Hub every 250 ms (configurable). Contains the current ship state.

**Access type:** Read-only. Flight Hub never writes to this file.

### `Journal.<date>.<session>.log`

**Location:** Same directory as `Status.json`.

**Purpose:** Tailed by Flight Hub for new JSONL event lines. Only a small subset of event types are read; all others are ignored.

**Access type:** Read-only, append-safe.

### No Other Files

Flight Hub does **not** modify Elite Dangerous installation files, `AppConfig.xml`, settings, or any file outside the journal directory.

---

## Status.json → BusSnapshot Field Mapping

### Ship Configuration Flags

| `Status.json` Flags bit | Meaning | `BusSnapshot` field |
|-------------------------|---------|---------------------|
| 0 — Docked | Docked at station | `validity.position_valid = false` |
| 1 — Landed | Landed on planet | `validity.position_valid = false` |
| 2 — GearDown | Landing gear deployed | `config.gear.{nose,left,right} = Down` |
| 8 — LightsOn | External lights on | `config.lights.nav = true`, `config.lights.landing = true` |
| 16 — InSRV | In surface vehicle | `validity.position_valid = false` |

### Fuel

| `Status.json` field | `BusSnapshot` field | Conversion |
|---------------------|---------------------|------------|
| `Fuel.FuelMain` | `config.fuel["main"]` (%) | `FuelMain / (FuelMain + FuelReservoir) * 100` |

### Validity

| Condition | `position_valid` | `safe_for_ffb` |
|-----------|-----------------|----------------|
| In-flight (normal space or supercruise) | `true` | `false` |
| Docked, Landed, or InSRV | `false` | `false` |

> `safe_for_ffb` is always `false` — `Status.json` does not expose attitude data.

### Fields Parsed but Not Yet Propagated

`Pips [u8; 3]`, `FireGroup u32`, `GuiFocus u32`, `Cargo f32`, `LegalState String`, and remaining `Flags` bits are deserialised and available for future use.

---

## Journal Event → Adapter State Mapping

| Journal event | Effect |
|---------------|--------|
| `LoadGame` | `current_ship` ← `Ship` |
| `Location` | `current_system` ← `StarSystem`; `navigation.active_waypoint` updated |
| `FsdJump` | `current_system` ← `StarSystem`; `docked_station` cleared |
| `Docked` | `docked_station` ← `StationName`; `current_system` ← `StarSystem` |
| `Undocked` | `docked_station` cleared |
| `Touchdown` | Latitude/longitude parsed (available for future mapping) |
| `Liftoff` | Latitude/longitude parsed (available for future mapping) |
| `RefuelAll` | Fuel amount parsed (available for future mapping) |

The current star system name is surfaced in `navigation.active_waypoint` (e.g., `"Sol"`, `"Colonia"`).

---

## Setup

### Automatic (Recommended)

1. Launch Flight Hub (`flightd`).
2. Start Elite: Dangerous.
3. Flight Hub auto-detects the journal directory from `%USERPROFILE%\Saved Games\Frontier Developments\Elite Dangerous` and begins reading.

No installation, mods, or scripts are required.

### Manual Directory Override

If auto-detection fails:

```toml
[elite]
journal_dir = "D:/Games/Saves/Elite Dangerous"
```

---

## Limitations

### No Attitude or Velocity Data

Elite: Dangerous does not provide pitch, roll, heading, or velocity in `Status.json`. The following `BusSnapshot` fields remain at their defaults:

- `kinematics.{pitch, bank, heading, ias, tas, vertical_speed, g_force, aoa}`

Force feedback effects based on attitude are **not available** for Elite: Dangerous.

### No Companion API (Planned)

Frontier's optional REST Companion API (`https://companion.orerve.net`) provides richer data including ship loadout, market prices, and CMDR profile. It requires OAuth 2.0 authentication via Frontier's auth service.

**Status:** Not yet implemented. Planned as a future enhancement.

When available it will provide:

| Companion endpoint | Data |
|--------------------|------|
| `GET /profile` | CMDR name, credits, ship loadout |
| `GET /market` | Commodity prices at current station |
| `GET /shipyard` | Ships available at current station |
| `GET /outfitting` | Modules available at current station |

---

## Revert

To disable Flight Hub's Elite: Dangerous integration:

1. Stop Flight Hub (`flightctl stop`).
2. Optionally set `journal_dir` to a non-existent path in configuration to prevent auto-detection.

No game files are modified; there is nothing to revert on the game side.

---

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| No snapshots published | `Status.json` not found | Verify journal directory path; ensure game has been launched at least once |
| `current_system` empty | No `Location`/`FsdJump` events processed | Load into a game session; the `Location` event fires on game load |
| Gear state not updating | Status flags not changing | Verify the adapter is running (`flightctl status --elite`) |

---

**Last Updated:** 2026-02-24
**Supported Game Version:** Elite Dangerous 4.x (Odyssey)
