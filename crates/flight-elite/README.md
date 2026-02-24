# flight-elite

Elite: Dangerous journal and `Status.json` adapter for [OpenFlight](https://github.com/flighthub/openflight).

## Overview

Elite: Dangerous writes live game-state data to two file-based APIs in the player's saved-games folder:

| File | Update frequency | Content |
|------|-----------------|---------|
| `Status.json` | ~4 Hz | Ship flags, pips, fuel, cargo, GUI focus |
| `Journal.<date>.<session>.log` | Event-driven (JSONL) | Typed events (FSD jumps, docking, etc.) |

This crate provides:

- **`EliteAdapter`** — polls `Status.json` at a configurable rate and publishes `BusSnapshot`s on the flight bus.
- **`JournalReader`** — discovers the latest journal log, tails new events, and parses relevant ones.
- **`apply_journal_event`** — apply parsed journal events to the adapter to update star system / docking context.

## Default Journal Directory

| Platform | Path |
|----------|------|
| Windows | `%USERPROFILE%\Saved Games\Frontier Developments\Elite Dangerous` |
| Linux (Proton/Wine) | `~/.steam/steam/steamapps/compatdata/<appid>/pfx/drive_c/users/steamuser/Saved Games/Frontier Developments/Elite Dangerous` |

The path is auto-detected by `EliteConfig::default_journal_dir()`.

## Status.json Fields Mapped

| `Status.json` field | `BusSnapshot` field | Notes |
|---------------------|---------------------|-------|
| `Flags` bit 0 (`Docked`) | `validity.position_valid = false` | Docked → not in-flight |
| `Flags` bit 2 (`GearDown`) | `config.gear.*` | `GearPosition::Down` / `Up` |
| `Flags` bit 8 (`LightsOn`) | `config.lights.nav`, `config.lights.landing` | Both nav and landing lights |
| `Fuel.FuelMain` + `FuelReservoir` | `config.fuel["main"]` | Fraction of main/(main+reserve) expressed as 0–100% |
| `Flags` bit 16 (`InSRV`) | `validity.position_valid = false` | SRV mode → not in-flight |

Fields without a direct BusSnapshot equivalent (Pips, GuiFocus, FireGroup, LegalState, Cargo) are
parsed but not yet propagated; they are available on `StatusJson` for future use.

## Journal Events Handled

| Event | Effect on adapter state |
|-------|------------------------|
| `LoadGame` | Updates `current_ship` |
| `Location` | Updates `current_system`; sets `navigation.active_waypoint` |
| `FsdJump` | Updates `current_system`; clears `docked_station` |
| `Docked` | Sets `docked_station` + `current_system` |
| `Undocked` | Clears `docked_station` |
| `Touchdown` / `Liftoff` | Parsed; latitude/longitude available if needed |
| `RefuelAll` | Parsed; fuel amount available if needed |

Unknown or irrelevant event lines are silently skipped.

## Configuration

```toml
[elite]
journal_dir  = "C:/Users/Alice/Saved Games/Frontier Developments/Elite Dangerous"
poll_interval_ms = 250    # How often to re-read Status.json
bus_max_rate_hz  = 4.0    # Maximum rate to publish BusSnapshots
```

## Quick Start

```rust
use flight_elite::{EliteAdapter, EliteConfig};

let mut adapter = EliteAdapter::new(EliteConfig::default());
adapter.start().unwrap();

loop {
    // Process any new journal events first.
    for event in adapter.journal_reader.read_new_events().unwrap() {
        adapter.apply_journal_event(&event);
    }
    // Then poll Status.json.
    if let Ok(Some(snapshot)) = adapter.poll_once().await {
        println!("snapshot: {:?}", snapshot.sim);
    }
    tokio::time::sleep(adapter.config.poll_interval).await;
}
```

## Limitations

- **No attitude data** — Elite Dangerous does not expose pitch/roll/heading in `Status.json`.
  `validity.safe_for_ffb` is always `false` for this adapter.
- **No velocity** — Speed data is not available. `kinematics` fields remain at their defaults.
- **No Companion API** — Frontier's optional REST API (market data, ship loadout, CMDR profile)
  is not yet implemented. See `docs/explanation/integration/elite.md` for details.
- **Linux path varies by Proton version** — Use `EliteConfig { journal_dir: <custom path>, .. }` to
  override auto-detection if needed.

## License

MIT OR Apache-2.0
