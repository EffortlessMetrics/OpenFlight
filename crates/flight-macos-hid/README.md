# flight-macos-hid

macOS IOKit/HID device layer for [Flight Hub](https://flight-hub.dev).

Provides a platform-consistent API for HID device enumeration and report I/O. On **macOS** the implementation is backed by IOKit's `IOHIDManager`. On other platforms all methods return `HidError::UnsupportedPlatform` so the workspace compiles everywhere while the real port lives behind `#[cfg(target_os = "macos")]`.

## Features

- `HidManager`: enumerate, filter, and open HID devices by usage page/usage
- `HidDevice`: read input reports and write output/feature reports
- `MacosClock`: high-resolution monotonic clock backed by `mach_absolute_time`
- Cross-compiles on Windows/Linux with stub implementations

## Usage

```rust
use flight_macos_hid::{HidManager, HidError};

fn main() -> Result<(), HidError> {
    let mut mgr = HidManager::new()?;
    // Match joysticks: usage page 0x01, usage 0x04
    mgr.set_device_matching(0x01, 0x04);
    mgr.open()?;
    for dev in mgr.devices() {
        println!("{:04x}:{:04x} – {}", dev.vendor_id, dev.product_id, dev.product_string);
    }
    Ok(())
}
```

## macOS Live Port

The live IOKit bindings are scaffolded behind commented-out dependencies in `Cargo.toml`. To enable the full port uncomment `io-kit-sys` and `core-foundation` and add them to the workspace `[dependencies]`.

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT license](../../LICENSE-MIT) at your option.
