---
doc_id: DOC-HOWTO-XPLANE
kind: how-to
area: integration
status: active
links:
  requirements:
    - REQ-42
  tasks: []
  adrs: []
---

# How to Set Up X-Plane Integration

This guide explains how to configure X-Plane 11 or X-Plane 12 to stream
telemetry to Flight Hub (OpenFlight) and optionally install the Flight Hub
plugin for enhanced aircraft detection.

---

## Prerequisites

| Requirement | Notes |
|---|---|
| X-Plane 11 or 12 | Tested with XP 11.55+ and XP 12.x |
| Flight Hub service (`flightd`) | Must be running before X-Plane starts |
| Local network or loopback | Default: `127.0.0.1`, port `49000` |

---

## Part 1 — UDP DataRef Output (Required)

Flight Hub reads X-Plane data via X-Plane's built-in UDP output. No plugin is
required for basic telemetry.

### 1.1 Enable UDP Data Output in X-Plane

1. Open X-Plane → **Settings** → **Data Output**.
2. In the **Data Output** tab, enable the following data groups by ticking
   the **Network via UDP** checkbox for each row:

   | Group # | Description |
   |---|---|
   | 3 | Speeds (IAS, TAS, ground speed) |
   | 4 | Mach, VVI, G-load |
   | 16 | Pitch, roll, headings |
   | 17 | Angle of attack, side-slip |
   | 18 | Engine RPM / N1 |
   | 20 | Gear deployment |
   | 21 | Flaps / spoilers |

3. Set the **IP address** to `127.0.0.1` (or the IP of the Flight Hub host
   if running on a separate machine).
4. Set the **Port** to `49000` (default; matches `XPlaneAdapterConfig::port`).
5. Click **Done**.

### 1.2 Verify Connectivity

Start `flightd` and load a flight in X-Plane. Check the Flight Hub log for:

```
INFO flight_xplane::adapter: X-Plane UDP connected, aircraft: C172
```

If the aircraft shows as `unknown`, the aircraft DataRefs have not been
received yet. Wait a few seconds or check that data groups 3/4/16/17/18 are
enabled.

---

## Part 2 — Flight Hub Adapter Configuration (Optional)

By default, the adapter uses `127.0.0.1:49000`. To change these:

### Configuration file (`flightd.toml`)

```toml
[adapters.xplane]
host = "127.0.0.1"       # X-Plane host IP
port = 49000             # UDP receive port
publish_hz = 30          # How often to push BusSnapshot (Hz)
enable_plugin = false    # Set true when the plugin is installed
reconnect_interval_s = 5
```

Place `flightd.toml` in:

| Platform | Path |
|---|---|
| Windows | `%APPDATA%\FlightHub\flightd.toml` |
| Linux | `~/.config/flight-hub/flightd.toml` |

### CLI

```sh
# Check current X-Plane adapter status
flightctl sim status xplane

# Reload configuration without restarting
flightctl reload
```

---

## Part 3 — Flight Hub Plugin (Optional, Enhanced Aircraft Detection)

The Flight Hub X-Plane plugin provides:
- Precise aircraft ICAO code and title (not just DataRef approximation)
- Aircraft change events (when you switch aircraft mid-session)
- Protected DataRef access (XPLM APIs not available via UDP)

### 3.1 Build the Plugin

The plugin source is in `crates/flight-xplane-plugin/`. It compiles to a
`.xpl` shared library.

```sh
# Windows
cargo build --release -p flight-xplane-plugin --target x86_64-pc-windows-msvc
# Output: target/x86_64-pc-windows-msvc/release/flight_xplane_plugin.dll

# Linux
cargo build --release -p flight-xplane-plugin --target x86_64-unknown-linux-gnu
# Output: target/x86_64-unknown-linux-gnu/release/libflight_xplane_plugin.so

# macOS (universal binary)
cargo build --release -p flight-xplane-plugin --target aarch64-apple-darwin
cargo build --release -p flight-xplane-plugin --target x86_64-apple-darwin
lipo -create \
  target/aarch64-apple-darwin/release/libflight_xplane_plugin.dylib \
  target/x86_64-apple-darwin/release/libflight_xplane_plugin.dylib \
  -output mac.xpl
```

### 3.2 Install the Plugin

Create a **fat plugin** directory structure in X-Plane's plugin folder:

```
<X-Plane root>/
  Resources/
    plugins/
      FlightHub/
        win.xpl    ← Windows build (renamed .dll)
        lin.xpl    ← Linux build (renamed .so)
        mac.xpl    ← macOS build (renamed .dylib)
```

**Windows:**
```powershell
$xp = "C:\X-Plane 12"
New-Item "$xp\Resources\plugins\FlightHub" -ItemType Directory -Force
Copy-Item "target\x86_64-pc-windows-msvc\release\flight_xplane_plugin.dll" `
          "$xp\Resources\plugins\FlightHub\win.xpl"
```

**Linux:**
```sh
XP="$HOME/X-Plane 12"
mkdir -p "$XP/Resources/plugins/FlightHub"
cp target/x86_64-unknown-linux-gnu/release/libflight_xplane_plugin.so \
   "$XP/Resources/plugins/FlightHub/lin.xpl"
```

### 3.3 Enable Plugin in Flight Hub

Set `enable_plugin = true` in `flightd.toml`:

```toml
[adapters.xplane]
enable_plugin = true
```

The plugin connects to `127.0.0.1:52000` on startup. `flightd` must be
running before X-Plane is launched so the plugin can connect.

### 3.4 Verify Plugin Connection

After loading X-Plane with the plugin installed, check the Flight Hub log for:

```
INFO flight_xplane::plugin: Plugin interface listening on 127.0.0.1:52000
INFO flight_xplane::plugin: Plugin connected from 127.0.0.1:XXXXX
INFO flight_xplane::plugin: Plugin handshake successful: version=0.x.x, status=ready
```

In X-Plane → **Plugins** menu → **Plugin Manager**, the `FlightHub` plugin
should be listed as enabled.

---

## Troubleshooting

| Symptom | Check |
|---|---|
| No telemetry received | Verify UDP data groups are enabled; check firewall allows port 49000 inbound |
| Aircraft shows as `unknown` | Groups 3/4/16 may be missing; wait a few seconds for aircraft DataRefs |
| Plugin not connecting | Ensure `flightd` is started first; check port 52000 is not in use |
| Plugin listed but disabled in XP | Build target mismatch (e.g. x86 XP + x64 plugin); rebuild for correct arch |
| High latency warnings | Reduce `publish_hz`, or move X-Plane and Flight Hub to same machine |

---

## Reference

- `XPlaneAdapterConfig` defaults: `crates/flight-xplane/src/adapter.rs`
- Plugin interface protocol: `crates/flight-xplane/src/plugin.rs`
- Plugin crate: `crates/flight-xplane-plugin/`
- Requirement: REQ-42
