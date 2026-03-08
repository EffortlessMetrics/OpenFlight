# flight-dcs-bios

DCS-BIOS binary protocol support for cockpit builders interfacing with DCS World.

DCS-BIOS is a protocol used by cockpit builders to interface with DCS World's
clickable cockpit controls through a serial/UDP binary protocol. This crate
provides:

- **Frame parsing** — decode the DCS-BIOS export stream (sync bytes + address/length/data updates)
- **State tracking** — maintain the 65536-byte cockpit memory map with change detection
- **Command building** — generate import protocol commands (plain-text `CONTROL VALUE\n`)
- **Module definitions** — pre-built control databases (F/A-18C Hornet included)

## Protocol overview

- **Export** (DCS → external): Binary frames with sync sequence `0x55 0x55 0x55 0x55`,
  followed by `(address_u16, length_u16, data)` update segments.
- **Import** (external → DCS): Newline-terminated plain text: `CONTROL_NAME VALUE\n`

## Quick start

```rust
use flight_dcs_bios::{DcsBiosState, parse_frame, modules};

// Load F/A-18C module definitions
let module = modules::fa18c::fa18c_module();

// Parse incoming binary data
let updates = parse_frame(&received_data).unwrap();

// Apply to state tracker
let mut state = DcsBiosState::new();
for update in &updates {
    state.apply_update(update);
}

// Read a control value
if let Some(value) = state.read_integer(&module, "MASTER_ARM_SW") {
    println!("Master Arm: {value}");
}
```
