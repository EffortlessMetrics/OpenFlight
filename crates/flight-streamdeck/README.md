# Flight Hub StreamDeck Plugin

StreamDeck plugin integration for Flight Hub providing real-time telemetry display and aircraft control actions.

## Features

- **Version Compatibility**: Supports StreamDeck app versions 5.3.0 - 6.4.x with graceful degradation
- **Sample Profiles**: Pre-built profiles for GA, Airbus, and Helicopter aircraft
- **Real-time Telemetry**: 30-60Hz telemetry updates with configurable rate limiting
- **Event Round-trip Testing**: Comprehensive verify tests for reliability
- **Local Web API**: HTTP API for StreamDeck plugin communication

## Installation

### Prerequisites

- StreamDeck app version 5.3.0 or later (6.0.0+ recommended for full features)
- Flight Hub service running
- Available port 8080 (configurable)

### StreamDeck Plugin Installation

1. **Download Plugin**: Download the Flight Hub StreamDeck plugin from the releases page
2. **Install Plugin**: Double-click the `.streamDeckPlugin` file to install
3. **Verify Installation**: The plugin should appear in the StreamDeck app actions list

### Configuration

The plugin automatically connects to Flight Hub on startup. Default configuration:

- **API Port**: 8080
- **Host**: 127.0.0.1 (localhost only)
- **Update Rate**: 30Hz telemetry updates
- **Timeout**: 30 second request timeout

## Port Requirements

### Default Ports

| Service | Port | Protocol | Purpose |
|---------|------|----------|---------|
| StreamDeck API | 8080 | HTTP | Plugin communication |
| Flight Hub IPC | Named Pipes/UDS | IPC | Internal communication |

### Port Configuration

The API port can be configured in the Flight Hub service configuration:

```toml
[streamdeck]
port = 8080
host = "127.0.0.1"
cors_origins = ["http://localhost:*", "https://localhost:*", "streamdeck://"]
```

### Firewall Configuration

**Windows**: Allow inbound connections on port 8080 for `flightd.exe`
**Linux**: Ensure port 8080 is not blocked by iptables/ufw

## Sample Profiles

### General Aviation (GA)
- Landing lights toggle
- Navigation lights toggle
- Landing gear control
- Flaps control

### Airbus
- Autopilot master
- Altitude hold
- Heading hold
- Approach mode

### Helicopter
- Engine start/stop
- Rotor brake
- Collective friction
- Anti-torque trim reset

## API Endpoints

### Version Check
```http
POST /api/v1/version/check
Content-Type: application/json

{
  "app_version": "6.2.0",
  "plugin_uuid": "com.flighthub.streamdeck"
}
```

### Telemetry Data
```http
GET /api/v1/telemetry?fields=ias,altitude,heading
```

### Profile Management
```http
GET /api/v1/profiles
GET /api/v1/profiles/ga
GET /api/v1/profiles/airbus
GET /api/v1/profiles/helo
```

### Health Check
```http
GET /api/v1/health
```

## Version Compatibility

### Fully Supported (6.0.0 - 6.4.x)
- All features available
- WebSocket API support
- Property Inspector support
- Multi-actions support

### Partially Supported (5.3.0 - 5.9.x)
- Basic actions available
- Profile support
- Limited multi-actions
- No Property Inspector

### Unsupported (< 5.3.0, >= 7.0.0)
- Plugin will not load
- User guidance provided for upgrade

## Troubleshooting

### Plugin Not Loading
1. Check StreamDeck app version compatibility
2. Verify Flight Hub service is running
3. Check port 8080 is not in use by another application
4. Review StreamDeck app logs for errors

### Connection Issues
1. Verify Flight Hub service is accessible on localhost:8080
2. Check firewall settings
3. Ensure no proxy/VPN interference
4. Test API endpoints manually with curl/browser

### Performance Issues
1. Reduce telemetry update rate in configuration
2. Check system resource usage
3. Verify network latency to Flight Hub service
4. Review StreamDeck app performance settings

### Version Warnings
1. Upgrade StreamDeck app to 6.0.0 or later for full features
2. Check compatibility matrix in plugin settings
3. Review feature availability for current version

## Development

### Building from Source

```bash
# Build the plugin
cargo build --release -p flight-streamdeck

# Run tests
cargo test -p flight-streamdeck

# Run with logging
RUST_LOG=debug cargo run --bin flight-streamdeck-server
```

### Testing

```bash
# Unit tests
cargo test -p flight-streamdeck

# Integration tests
cargo test -p flight-streamdeck --test integration

# Verify test
cargo test -p flight-streamdeck verify_test
```

### API Testing

```bash
# Health check
curl http://localhost:8080/api/v1/health

# Version check
curl -X POST http://localhost:8080/api/v1/version/check \
  -H "Content-Type: application/json" \
  -d '{"app_version": "6.2.0", "plugin_uuid": "test"}'

# Get telemetry
curl http://localhost:8080/api/v1/telemetry
```

## Security

- **Local Only**: API binds to localhost only by default
- **No Authentication**: Relies on local system security
- **CORS Enabled**: Allows StreamDeck app origins only
- **No Data Collection**: No telemetry sent outside local system

## Support

For issues and support:

1. Check the troubleshooting section above
2. Review Flight Hub logs for errors
3. Test API endpoints manually
4. Submit issues with logs and system information

## License

Licensed under MIT OR Apache-2.0