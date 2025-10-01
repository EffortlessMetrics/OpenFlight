# Flight Hub DCS Export Integration

This crate provides DCS World integration for Flight Hub through user-installed Export.lua scripts.

## Features

- **MP-Safe Telemetry**: Respects DCS multiplayer integrity by blocking restricted features
- **Socket-Based Communication**: Local TCP socket for reliable data transfer
- **Version Negotiation**: Protocol versioning for compatibility
- **Session Detection**: Automatic SP/MP session detection
- **User-Controlled**: Export.lua is user-installed and user-controlled

## Architecture

```
DCS World → Export.lua → Socket Bridge → DCS Adapter → Flight Bus
```

### Components

- **Export.lua**: User-installed script in DCS Saved Games directory
- **Socket Bridge**: TCP server for DCS communication
- **MP Detector**: Session type detection and feature blocking
- **DCS Adapter**: Telemetry conversion and bus publishing

## Installation

### Automatic Installation (Recommended)

1. Run Flight Hub installer
2. Select "Install DCS Export" option
3. Restart DCS World

### Manual Installation

1. Generate Export.lua using Flight Hub
2. Copy to: `DCS.openbeta\Scripts\Export.lua`
3. If Export.lua exists, append the Flight Hub code
4. Restart DCS World

## What We Touch

Flight Hub DCS integration is designed to be minimal and transparent:

### Files Modified
- `Scripts/Export.lua` (user-installed, user-controlled)

### Network Connections
- Local socket connection to `127.0.0.1:7778`
- No external network access

### DCS APIs Used
- `LoGetSelfData()` - Aircraft position/attitude
- `LoGetIndicatedAirSpeed()` - Airspeed data
- `LoGetEngineInfo()` - Engine telemetry
- `LoGetPayloadInfo()` - Weapons data (SP only)
- `LoGetSnares()` - Countermeasures (SP only)
- `net.get_server_id()` - MP session detection

## Multiplayer Integrity

Flight Hub respects DCS multiplayer server integrity:

### Single Player
- All telemetry features available
- Full aircraft data access
- No restrictions

### Multiplayer
- Weapons data blocked
- Countermeasures data blocked
- RWR contacts blocked
- Basic flight data allowed
- Clear UI messaging when features are blocked

### Implementation
- Export.lua declares MP-safe vs blocked features
- Adapter refuses blocked features in MP sessions
- Session type detected automatically
- No code injection into DCS process

## Removal

To remove Flight Hub DCS integration:

1. Delete or rename `Scripts/Export.lua`
2. Restart DCS World

All changes are immediately reverted.

## Troubleshooting

### Connection Issues
1. Check Windows Firewall allows Flight Hub
2. Verify DCS Scripts folder exists
3. Check DCS.log for Lua errors

### MP Restrictions
- Some features disabled in MP for server integrity
- This is normal and expected behavior
- Features work normally in single-player

### Conflicts with Other Exports
- Flight Hub export is designed to coexist
- Uses legacy hook system for compatibility
- Contact support if conflicts occur

## Development

### Building
```bash
cargo build -p flight-dcs-export
```

### Testing
```bash
cargo test -p flight-dcs-export
```

### Features
- `default` - All standard features
- `test-utils` - Testing utilities

## Protocol

### Message Format
JSON messages over TCP, one per line:

```json
{
  "type": "Telemetry",
  "data": {
    "timestamp": 1234567890,
    "aircraft": "F-16C",
    "session_type": "SP",
    "data": { ... }
  }
}
```

### Handshake
```json
{
  "type": "Handshake", 
  "data": {
    "version": {"major": 1, "minor": 0},
    "features": ["telemetry_basic", "telemetry_engines"]
  }
}
```

### Version Compatibility
- Major version must match
- Minor version differences are compatible
- Feature negotiation handles capability differences

## Security

- Local-only communication (127.0.0.1)
- No external network access
- User-controlled Export.lua installation
- No DLL injection or process modification
- Respects DCS multiplayer policies

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.