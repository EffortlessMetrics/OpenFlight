# flight-xplane-plugin

X-Plane plugin (`.xpl`) that bridges X-Plane to the [Flight Hub](https://flight-hub.dev) plugin interface.

This crate compiles to a `cdylib` shared library that X-Plane loads as a native plugin. It spawns a background thread that connects to the Flight Hub plugin TCP server (`localhost:52000`) and relays DataRef values and aircraft info.

## Building

```sh
# Windows (.xpl = .dll renamed)
cargo build --release --target x86_64-pc-windows-msvc
copy target\x86_64-pc-windows-msvc\release\flight_xplane_plugin.dll ^
     "<X-Plane>/Resources/plugins/FlightHub/win.xpl"

# Linux
cargo build --release --target x86_64-unknown-linux-gnu
cp target/x86_64-unknown-linux-gnu/release/libflight_xplane_plugin.so \
   "<X-Plane>/Resources/plugins/FlightHub/lin.xpl"

# macOS (arm64 + x86_64 universal)
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin
lipo -create -output mac.xpl \
     target/aarch64-apple-darwin/release/libflight_xplane_plugin.dylib \
     target/x86_64-apple-darwin/release/libflight_xplane_plugin.dylib
cp mac.xpl "<X-Plane>/Resources/plugins/FlightHub/mac.xpl"
```

## Plugin Install Path

Place the built `.xpl` in:
```
<X-Plane 12>/Resources/plugins/FlightHub/
  win.xpl   (Windows)
  lin.xpl   (Linux)
  mac.xpl   (macOS)
```

## Features

- Connects to Flight Hub service via TCP on startup
- Streams DataRef values: airspeed, altitude, heading, pitch, roll, engine data
- Relays aircraft ICAO type and phase-of-flight hints
- Automatic reconnection on disconnect

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT license](../../LICENSE-MIT) at your option.
