# Compatibility Manifests

Machine-readable compatibility manifests for OpenFlight hardware and game integrations.

## Structure

```
compat/
  devices/
    <vendor>/
      <device>.yaml   — HID device manifest (VID/PID, axes, buttons, quirks, tier)
  games/
    <game>.yaml       — Game integration manifest (mechanism, features, known issues)
```

## Support Tiers

| Tier | Meaning |
|------|---------|
| 1 | Automated trace tests + recent HIL validation |
| 2 | Automated tests (no HIL) + community confirmation |
| 3 | Compiles / best-effort — no guarantees |

## Generating the compatibility matrix

```bash
cargo xtask gen-compat
```

This produces `COMPATIBILITY.md` from all manifests in this directory.

## Schema

All files use `schema_version: "1"`. Required fields:

### Device manifest
- `device.name`, `device.vendor`, `device.usb.vendor_id`, `device.usb.product_id`
- `capabilities.axes`, `capabilities.buttons`, `capabilities.force_feedback`
- `support.tier`

Optional fields (all manifests may include these):

| Field | Type | Description |
|-------|------|-------------|
| `driver` | string | Driver/software requirement. `standard_hid` = plug and play; otherwise names the required vendor tool (e.g. `virpil_vpc`, `vkb_devcfg`, `vpforce_companion`). |
| `firmware.min_version` | string | Minimum tested firmware version (semver). |
| `firmware.max_version` | string or null | Maximum tested firmware version; `null` means all newer versions are expected to work. |
| `firmware.notes` | string | Firmware-specific quirks or update instructions. |
| `tested_games` | list | Simulators this device has been tested with. Each entry: `game` (display name), `integration` (mechanism), `notes` (free text). |

### Game manifest
- `game.name`, `game.id`, `integration.mechanism`, `integration.crate`
- `features.telemetry_read`, `features.control_injection`
- `test_coverage.hil`
