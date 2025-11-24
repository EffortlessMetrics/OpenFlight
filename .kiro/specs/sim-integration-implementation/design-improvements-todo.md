# Design Document Improvements TODO

This document tracks the remaining improvements to apply to the design document based on review feedback.

## Completed
- ✅ Separated BusSnapshot (ergonomic) from BusSnapshotRaw (ABI-safe POD)
- ✅ Added explicit Bus Publishing Model section
- ✅ Enhanced SanityGate with explicit state machine and transitions
- ✅ Reduced SanityGate memory footprint (store minimal state, not full snapshot)

## Remaining Improvements

### MSFS Adapter
- [ ] Separate identity (TITLE, ATC_TYPE, ATC_MODEL) into low-rate subscription
- [ ] Add note about draining SimConnect dispatch queue each tick (not just one message)
- [ ] Label SimVar table as illustrative, link to canonical mapping doc
- [ ] Clarify that unit strings should match actual SimConnect values in mapping doc

### X-Plane Adapter
- [ ] Add explicit note that aircraft identity is unavailable/inferred in UDP-only mode
- [ ] Document that true profile switching requires plugin or identity-only companion plugin
- [ ] Add table of required Data Output indices to enable (3, 4, 16, 17, 18, 21, etc.)
- [ ] Include recommended UDP rate setting for X-Plane UI

### DCS Adapter
- [ ] Fix Export.lua chaining pattern to properly call previous hooks:
  ```lua
  local PrevLuaExportStart = LuaExportStart
  function LuaExportStart()
      if PrevLuaExportStart then PrevLuaExportStart() end
      FlightHub.LuaExportStart()
  end
  ```
- [ ] Clarify MP-limited mode: keep self-aircraft data valid, only annotate MP status
- [ ] Define whitelist: always allowed (self attitude/velocities/g-load/IAS/TAS/AoA) vs never (world objects/RWR/sensors/weapons)
- [ ] Add note that JSON is v1 for debuggability, can swap to binary encoding later

### FFB Implementation
- [ ] Clarify torque direction mapping: per-axis torque_nm for pitch/roll mapped to DirectInput X/Y axes
- [ ] Add device calibration section: query firmware for max_torque_nm, min_period_us, thermal/current limits
- [ ] Store per-device calibration in config file, not hardcoded constants
- [ ] Add explicit fault_start_time for 50ms ramp-to-zero enforcement regardless of jitter
- [ ] Document XInput rumble as coarse vibration only, not full torque modeling

### Runtime Loops
- [ ] Add note that busy-spin duration (50-80μs) is configurable for jitter vs CPU trade-off
- [ ] Clarify timeBeginPeriod(1) fallback: measure jitter with/without, enable only when necessary
- [ ] Document that jitter tests run on hardware-backed runners (non-virtualized, fixed CPU governor)
- [ ] Add note that generic CI nodes may skip or downgrade jitter tests

### Packaging
- [ ] Clarify InstallScope: core can be per-user, but sim integrations and virtual drivers may require per-machine/admin
- [ ] Add ViGEmBus (or chosen virtual controller driver) to third-party components inventory
- [ ] Document install flow: bundled vs user-installed prerequisite, graceful degradation if missing

### Observability
- [ ] Add "Telemetry & Metrics" section with:
  - Naming convention (sim.msfs.update_hz, ffb.usb_write_latency_p99, etc.)
  - Cardinality discipline (per-adapter, not per-aircraft)
  - Export destination (Prometheus, in-process ring buffer, log-structured)

### Version Scope Clarity
- [ ] Mark X-Plane plugin section as "Future Plugin Design (v2)"
- [ ] Mark Linux FFB code as "v2 Candidate Implementation"
- [ ] Explicitly mark any kernel-driver concepts as out-of-scope for v1

## Notes

These improvements don't change the spirit of the design—they make the ABI sane, the concurrency story explicit, and the sim-specific edges and safety behavior match requirements more tightly.

Priority order:
1. ABI/concurrency (BusSnapshotRaw, publishing model) - DONE
2. Sim-specific edge cases (MSFS identity, X-Plane aircraft detection, DCS chaining)
3. FFB safety details (calibration, fault timing)
4. Observability and packaging clarity
