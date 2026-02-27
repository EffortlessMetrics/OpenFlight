# flight-vr

VR headset adapter for OpenFlight. Provides a unified interface for polling head pose data from VR hardware via the `VrBackend` trait. Supports mock backends for testing.

## Overview

This crate abstracts over VR runtime APIs (OpenVR/SteamVR, OpenXR) behind a single `VrBackend` trait, delivering 6DOF head pose (`HeadPose`) and tracking quality information to the OpenFlight axis engine.

## Usage

```rust
use flight_vr::{VrAdapter, MockVrBackend, VrSnapshot, HeadPose, TrackingQuality};

let snapshots = vec![VrSnapshot {
    pose: HeadPose::zero(),
    quality: TrackingQuality::Good,
    is_worn: true,
}];
let mut adapter = VrAdapter::new(MockVrBackend::new_connected(snapshots));
let snapshot = adapter.update().unwrap();
println!("yaw: {}", snapshot.pose.yaw);
```

## Features

| Feature | Description |
|---------|-------------|
| `default` | No hardware dependencies — mock backend only |
| `openvr` | *(planned)* OpenVR/SteamVR polling via the `openvr` crate |
