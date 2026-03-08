---
doc_id: DOC-HOWTO-ADDING-DEVICE
title: "How to Add Support for a New Device"
status: active
category: how-to
group: flight-hid
requirements:
  - REQ-3
---

# How to Add Support for a New Device

This guide walks through adding first-class support for a new HID flight
peripheral (stick, throttle, panel, or pedals) to OpenFlight.

## Overview

Adding a device involves four steps:

1. Create a HOTAS / device crate (or extend an existing vendor crate)
2. Implement HID report parsing
3. Add a device manifest to `compat/devices/`
4. Write tests
5. Open a pull request

## Step 1 — Create or Extend a Device Crate

Device crates live under `crates/` and follow the naming convention
`flight-hotas-<vendor>` (for sticks/throttles) or `flight-panels-<vendor>`
(for instrument panels).

### Scaffold a New Crate

```bash
cargo init crates/flight-hotas-myfirm --lib
```

Add the crate to the workspace in the root `Cargo.toml`:

```toml
[workspace]
members = [
    # ... existing members ...
    "crates/flight-hotas-myfirm",
]
```

Use workspace dependencies — **never** pin crate-local versions for
shared dependencies:

```toml
# crates/flight-hotas-myfirm/Cargo.toml
[dependencies]
flight-hid-types = { workspace = true }
thiserror        = { workspace = true }
tracing          = { workspace = true }

[dev-dependencies]
flight-hid-support = { workspace = true }
```

### Crate Structure

Follow the established pattern used by `flight-hotas-saitek` and
`flight-hotas-thrustmaster`:

```
crates/flight-hotas-myfirm/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs           # Re-exports, error type, feature gates
    ├── input.rs         # HID report → axis/button parsing
    ├── health.rs        # Device health checks
    ├── traits.rs        # Optional output traits (LED, MFD, RGB)
    └── my_device.rs     # Device-specific implementation
```

### Error Type

Define a crate-level error enum:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HotasError {
    #[error("device not found (VID={vid:#06x} PID={pid:#06x})")]
    DeviceNotFound { vid: u16, pid: u16 },

    #[error("USB communication error: {0}")]
    UsbError(String),

    #[error("unverified protocol: {0}")]
    UnverifiedProtocol(String),

    #[error("operation not supported on this device")]
    NotSupported,

    #[error("invalid parameter: {0}")]
    InvalidParameter(String),
}
```

## Step 2 — Implement HID Report Parsing

The input module translates raw HID reports into normalised
`AxisFrame` values that the RT spine consumes.

### Identify the HID Report Layout

1. Obtain the USB vendor ID (`VID`) and product ID (`PID`).
2. Capture a raw HID report using a tool like
   [Wireshark](https://www.wireshark.org/) (USBPcap) or
   `hidapi`'s example programs.
3. Map byte offsets to axes, buttons, and hats. Document the layout
   in a comment block:

```rust
// MyFirm Stick Pro HID Report (8 bytes)
// ┌──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┐
// │  X   │  Y   │  Z   │ Rx   │ Btns │ Btns │ Hat  │ Rsvd │
// │ 0-FF │ 0-FF │ 0-FF │ 0-FF │ [0]  │ [1]  │      │      │
// └──────┴──────┴──────┴──────┴──────┴──────┴──────┴──────┘
```

### Parse and Normalise

```rust
/// Parse a raw HID input report into normalised axis values.
///
/// All axes are normalised to the range [-1.0, 1.0].
pub fn parse_report(buf: &[u8]) -> Result<InputSnapshot, HotasError> {
    if buf.len() < 8 {
        return Err(HotasError::InvalidParameter(
            "report too short".into(),
        ));
    }

    let x = normalise_u8(buf[0]);  // -1.0 .. 1.0
    let y = normalise_u8(buf[1]);
    // ...
    Ok(InputSnapshot { x, y, /* ... */ })
}

fn normalise_u8(raw: u8) -> f32 {
    (raw as f32 / 127.5) - 1.0
}
```

### Optional: Output Protocols

If the device supports LEDs, an MFD, or RGB zones, implement the
relevant trait behind a feature gate:

```toml
[features]
led = []
mfd = []
rgb = []
```

```rust
#[cfg(feature = "led")]
impl LedProtocol for MyDevice {
    fn set_led(&mut self, id: LedId, state: LedState) -> Result<(), HotasError> {
        // Build and send the HID output report
    }
}
```

> **Important:** Mark experimental output protocols as `UNVERIFIED`
> until they have been confirmed on real hardware.

## Step 3 — Add a Device Manifest

Every supported device has a YAML manifest in
`compat/devices/<vendor>/<device>.yaml`.

### Manifest Template

```yaml
schema_version: "1"

device:
  name: "MyFirm Stick Pro"
  vendor: "MyFirm"
  usb:
    vendor_id: 0x1234
    product_id: 0x5678
  hid:
    usage_page: 0x01      # Generic Desktop
    usage: 0x04           # Joystick

capabilities:
  axes:
    count: 4
    names: [x, y, z, rx]
    resolution_bits: 8
  buttons: 12
  hats: 1
  force_feedback: false

polling:
  expected_rate_hz: 125
  max_jitter_ms: 4

quirks: []

support:
  tier: 3                 # Best-effort until HIL-validated
  test_coverage:
    simulated: true
    hil: false
```

### Support Tiers

| Tier | Meaning |
|------|---------|
| 1 | Hardware-in-the-loop (HIL) validated |
| 2 | Automated / simulated tests pass |
| 3 | Best-effort (manifest exists, basic parsing) |

### Regenerate the Compatibility Matrix

```bash
cargo xtask gen-compat
```

This updates `COMPATIBILITY.md` from all manifests under `compat/`.

## Step 4 — Write Tests

### Unit Tests

Test the report parser with known byte sequences:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_centered_report() {
        let report = [0x80, 0x80, 0x80, 0x80, 0x00, 0x00, 0x00, 0x00];
        let snap = parse_report(&report).unwrap();
        assert!((snap.x).abs() < 0.02, "center should be near zero");
    }

    #[test]
    fn parse_full_deflection() {
        let report = [0xFF, 0x00, 0x80, 0x80, 0x00, 0x00, 0x00, 0x00];
        let snap = parse_report(&report).unwrap();
        assert!(snap.x > 0.98);
        assert!(snap.y < -0.98);
    }

    #[test]
    fn reject_short_report() {
        let short = [0x00; 4];
        assert!(parse_report(&short).is_err());
    }
}
```

### Integration Tests

If you have access to the physical device, add a hardware-in-the-loop
test behind `#[ignore]`:

```rust
#[test]
#[ignore] // Requires physical device
fn hil_read_real_device() {
    // Open HID device, read a report, assert non-zero axes
}
```

### Run Tests

```bash
# Unit tests only
cargo test -p flight-hotas-myfirm

# Include ignored HIL tests (device must be connected)
cargo test -p flight-hotas-myfirm -- --ignored
```

## Step 5 — Open a Pull Request

1. Ensure all checks pass:
   ```bash
   cargo xtask check
   cargo clippy -p flight-hotas-myfirm -- -D warnings
   ```

2. Branch from `main`:
   ```bash
   git checkout -b feat/hotas-myfirm
   ```

3. Commit with a descriptive message:
   ```bash
   git add -A
   git commit -m "feat(hotas): add MyFirm Stick Pro support

   - HID report parsing for 4 axes, 12 buttons, 1 hat
   - Device manifest (tier 3)
   - Unit tests for report parsing"
   ```

4. Push and open a PR against `main`.

## Reference

- [Supported Hardware](../reference/supported-hardware.md) — current
  device matrix
- [Compatibility README](../../compat/README.md) — manifest schema
  details
- [Flight HID Concepts](../explanation/flight-hid.md) — HID subsystem
  architecture
