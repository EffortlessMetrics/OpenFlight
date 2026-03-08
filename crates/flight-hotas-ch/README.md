# flight-hotas-ch

CH Products device support for OpenFlight.

Provides HID report parsers, axis presets, device profiles, and health monitoring
for CH Products flight peripherals:

- **CH Fighterstick** — joystick with 3 axes + twist, 32 buttons, 4 hats (VID `0x068E`, PID `0x00F3`)
- **CH Pro Throttle** — throttle with mini-stick, rotary dial, 24 buttons (PID `0x00F1`)
- **CH Pro Pedals** — rudder + differential toe brakes (PID `0x00F2`)
- **CH Combat Stick** — 3 axes + twist, 24 buttons, 1 hat (PID `0x00F4`)
- **CH Eclipse Yoke** — yoke with roll/pitch + throttle knob, 32 buttons (PID `0x0051`)
- **CH Flight Sim Yoke** — classic yoke, 20 buttons, 1 hat (PID `0x00FF`)

## Features

- `serde` — enables `Serialize`/`Deserialize` on all state and protocol types

## Example

```rust
use flight_hotas_ch::{parse_fighterstick, normalize_axis};

let report = [0x01, 0x00, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00, 0x00];
let state = parse_fighterstick(&report).unwrap();
let roll = normalize_axis(state.x);
println!("{state}"); // uses Display impl
```
