# flight-openxr

OpenXR head tracking adapter for OpenFlight.

Reads HMD pose data from an OpenXR runtime and publishes it to the OpenFlight bus as head tracking snapshots (6DOF: x/y/z in metres, yaw/pitch/roll in radians).

## Design

The crate is built around the [`OpenXrRuntime`] trait so that the real OpenXR session can be swapped for a [`MockRuntime`] in tests — no HMD hardware or OpenXR loader is required to compile or test.

```
OpenXrRuntime (trait)
 ├── RealRuntime      (future: wraps the openxr crate)
 └── MockRuntime      (deterministic fake for tests)
         │
    OpenXrAdapter     (drives the runtime, owns state machine)
         │
    HeadPose          (6DOF snapshot published to the bus)
```

## Usage

### Production (bring your own runtime)

```rust,no_run
use flight_openxr::{OpenXrAdapter, OpenXrRuntime, HeadPose, OpenXrError};

struct MyRuntime { /* wraps openxr crate */ }

impl OpenXrRuntime for MyRuntime {
    fn initialize(&mut self) -> Result<(), OpenXrError> { todo!() }
    fn poll_pose(&mut self) -> Result<HeadPose, OpenXrError> { todo!() }
    fn shutdown(&mut self) {}
}

let mut adapter = OpenXrAdapter::new(MyRuntime { /* … */ });
adapter.initialize().unwrap();
let pose = adapter.poll();
```

### Testing with MockRuntime

```rust
use flight_openxr::{OpenXrAdapter, MockRuntime, HeadPose, SessionState};

let poses = vec![
    HeadPose { x: 0.1, y: 0.0, z: 0.0, yaw: 0.5, pitch: 0.0, roll: 0.0 },
    HeadPose::zero(),
];
let runtime = MockRuntime::new(poses);
let mut adapter = OpenXrAdapter::new(runtime);
adapter.initialize().unwrap();

assert_eq!(adapter.state(), SessionState::Running);
let pose = adapter.poll();
assert!((pose.x - 0.1).abs() < 1e-6);
```
