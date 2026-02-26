---
doc_id: DOC-HOWTO-WINGMAN
kind: how-to
area: simulation
status: active
links:
  requirements:
    - REQ-43
  tasks: []
  adrs: []
---

# Project Wingman Setup

**Flight Hub version:** 0.1 and later  
**Game version:** Project Wingman v1.0+ (Steam / itch.io)  
**Platform:** Windows 10/11 (64-bit)

---

## Overview

Project Wingman is an Unreal Engine 4 combat flight game. It exposes **no**
in-process telemetry API. Flight Hub supports it through two mechanisms:

1. **Process detection** – Flight Hub detects `ProjectWingman.exe` and
   activates your Wingman profile automatically.
2. **Virtual controller output** – Flight Hub routes processed HOTAS axis and
   button data to a virtual XInput gamepad that the game reads via SDL2.

---

## Prerequisites

### 1 – ViGEm Bus (virtual controller driver)

The stub virtual controller bundled with Flight Hub logs inputs but does **not**
create a real device. To route inputs into the game you must install the
**ViGEm Bus** kernel driver:

1. Download the latest installer from
   <https://github.com/nefarius/ViGEmBus/releases>.
2. Run `ViGEmBus_Setup_*.exe` and follow the prompts.
3. Reboot if prompted.

> A ViGEm-backed `VirtualController` implementation is planned for a future
> Flight Hub release. Until then, use the in-game axis-binding workflow below
> with your physical HOTAS directly, and rely on Flight Hub for profile
> management.

### 2 – SDL2 input (game requirement)

Project Wingman uses SDL2 for controller input. If the game does not detect
your joystick:

- Launch the game once with the `-SDL2.dll` workaround described in community
  guides (place a patched `SDL2.dll` beside `ProjectWingman.exe`).
- Alternatively, enable the **Steam Input** overlay and bind axes there.

---

## Flight Hub configuration

Flight Hub detects Project Wingman automatically; no manual configuration is
required. Verify detection is enabled in your profile:

```toml
# ~/.config/flight-hub/profile.toml  (or %APPDATA%\FlightHub\profile.toml)
[adapters]
enable_wingman = true          # default: true
```

### Launch order

1. Start **flightd** (Flight Hub service).
2. Launch **Project Wingman**.
3. Flight Hub detects `ProjectWingman.exe` within ~1 second and activates your
   Wingman profile.

---

## In-game axis binding

Because Flight Hub currently uses a stub virtual controller, bind your
**physical HOTAS** directly in-game:

1. From the main menu go to **Options → Controls**.
2. Select your HOTAS device.
3. Bind the following axes:

   | Axis | Recommended binding |
   |------|---------------------|
   | Pitch | Stick Y |
   | Roll | Stick X |
   | Throttle | Throttle Z |
   | Yaw | Stick Rz (twist) or rudder pedals |

4. Bind missile lock / weapons release / afterburner to your preferred buttons.
5. Save and exit.

---

## Verifying detection

```bash
flightctl sim status
```

Expected output when Project Wingman is running:

```
Simulator:  Project Wingman
State:      Connected
Adapter:    Wingman (presence-only; no telemetry API)
```

---

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| Game not detected | Ensure `enable_wingman = true` in profile; check that `ProjectWingman.exe` appears in Task Manager |
| Axes not responding in-game | Bind physical HOTAS directly (virtual controller is stub-only) |
| Stick drift | Apply deadzones via Flight Hub axis curves in the Wingman profile |
| Game freezes on SDL init | Try the `-SDL2.dll` workaround or disable Steam Input overlay |

---

## Known limitations

- **No telemetry** – all `BusSnapshot` validity flags are `false`. Autopilot
  sync and FFB features that depend on flight state are not available.
- **Virtual controller** – real XInput output requires ViGEm Bus plus the
  planned `vigem-client` integration (tracked in ROADMAP.md).
- **macOS / Linux** – Project Wingman is Windows-only; adapter is not built
  for other platforms.
