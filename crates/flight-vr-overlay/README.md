# flight-vr-overlay

VR overlay for [Flight Hub](https://flight-hub.dev) — in-cockpit notifications, profile status, and axis monitoring.

Renders a heads-up overlay inside VR headsets (OpenXR / SteamVR) showing real-time Flight Hub data while you fly.

## Architecture

```
Flight Hub Bus ──► OverlayService ──► OverlayRenderer (OpenXR / SteamVR)
                        │
                  NotificationQueue
                  OverlayState
```

## Features

- Push notifications with severity levels (info, warning, error) and configurable duration
- Live profile name and status display
- Axis position indicators (roll, pitch, throttle, yaw)
- `NullRenderer` for headless / test environments
- Async service driven by `OverlayService::spawn`

## Usage

```rust
use flight_vr_overlay::{OverlayConfig, OverlayService};
use flight_vr_overlay::renderer::NullRenderer;
use flight_vr_overlay::notification::Severity;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let handle = OverlayService::spawn(OverlayConfig::default(), NullRenderer::new());
    handle.notify("Profile loaded: MSFS-A320", Severity::Info, 4).await?;
    handle.shutdown().await;
    Ok(())
}
```

## Live VR Port

The live OpenXR/SteamVR renderer is behind the `openxr` feature flag. Stub `NullRenderer` is the default so the workspace compiles without a VR SDK.

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT license](../../LICENSE-MIT) at your option.
