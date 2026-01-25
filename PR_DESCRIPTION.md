# Pull Request: Migrate blackbox framing to postcard; unify time + units; fix reqwest TLS feature

## Why
- bincode 3.x is a tombstone; postcard framing gives explicit control + bounded decoding.
- unify bus timestamp semantics (monotonic) across adapters.
- prevent platform build break (reqwest feature mismatch).

## What changed
- postcard wire format: len-prefixed header/record/index; footer trailer; CRC checked on open.
- blackbox: index offsets documented as frame start; queue capacity recorded in stats; drop warning on first drop.
- flight_core::time: monotonic + unix base captured at start; unix_now derived without wall-clock jumps.
- flight_core::units: canonical conversions + angle normalization; adapters consume them.
- reqwest: workspace policy uses rustls-native-certs for 0.13.1.
- flight-updater: use rand::rngs::OsRng; remove rand_core feature mismatch.

## Safety surfaces
- bounded frame reads (MAX_* caps), checked range math, CRC verification on open.
- bounded queue + drop-tail behavior + counters + drop health warning.

## Glass Cockpit (target/perf-dashboard/latest.json, 2026-01-24 22:10:05Z)
| Metric | Value |
| --- | --- |
| jitter_p50_us | 150.0 |
| jitter_p99_us | 450.0 |
| hid_p99_us | 280.0 |
| deadline_misses | 2 |
| writer_drops | 0 |
| duration_s | 60.0 |
| platform | windows |

## Verification
- `rg -n "\bbincode\b" crates examples -S`
- `cargo tree -i bincode` (no package in graph)
- `cargo check -p flight-updater`
- `cargo check -p flight-core`
