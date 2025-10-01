# DCS Multiplayer Integrity Contract

This document outlines Flight Hub's approach to maintaining DCS multiplayer server integrity while providing useful telemetry data.

## Overview

Flight Hub respects the competitive integrity of DCS multiplayer servers by blocking access to sensitive data that could provide unfair advantages. This is implemented through automatic session detection and feature blocking.

## Session Detection

### Single Player Detection
- `player_count == 1`
- No server name present
- No multiplayer-specific fields

### Multiplayer Detection
- `session_type == "MP"` (explicit)
- Server name present
- `player_count > 1`
- Presence of coalition/side/group_id fields

### Unknown Sessions
- When session type cannot be determined
- Treated as multiplayer (apply restrictions)
- Ensures conservative approach

## Feature Classification

### MP-Safe Features (Always Available)
- **Basic Telemetry**: Position, attitude, airspeed, altitude
- **Navigation**: Waypoint data, course information
- **Engine Data**: RPM, temperature, fuel flow
- **Aircraft Config**: Gear, flaps, lights (non-tactical)
- **Environmental**: Weather, time of day

### MP-Blocked Features (SP Only)
- **Weapons Data**: Loadout, ammunition counts, weapon status
- **Countermeasures**: Chaff/flare counts, dispenser status
- **RWR Contacts**: Radar warning receiver data
- **Datalink**: Tactical data sharing information
- **Sensor Data**: Radar contacts, targeting information

## Implementation Details

### Export.lua Feature Flags
```lua
FlightHubExport.features = {
    telemetry_basic = true,         -- MP-safe
    telemetry_navigation = true,    -- MP-safe  
    telemetry_engines = true,       -- MP-safe
    telemetry_weapons = true,       -- MP-blocked
    telemetry_countermeasures = true, -- MP-blocked
    telemetry_rwr = true,           -- MP-blocked
}
```

### Runtime Blocking
```lua
-- Check MP restrictions
local is_mp = (session_type == "MP")
local mp_safe_mode = FlightHubExport.config.mp_safe_mode

-- Weapons data (MP-blocked)
if FlightHubExport.features.telemetry_weapons and (not is_mp or not mp_safe_mode) then
    -- Collect weapons data
end
```

### Adapter Validation
```rust
// Validate feature access
pub fn validate_feature(&self, feature: &str) -> Result<(), MpDetectionError> {
    if !self.is_feature_allowed(feature) {
        return Err(MpDetectionError::FeatureBlocked {
            feature: feature.to_string(),
            session_type: self.current_session_type(),
        });
    }
    Ok(())
}
```

## User Experience

### MP Session Banner
When connected to a multiplayer server:
```
DCS Multiplayer Session on [Server Name] - Some features are restricted for server integrity
```

### Blocked Feature Messages
When a blocked feature is requested:
```
Feature 'weapons telemetry' is not available in Multiplayer sessions for DCS multiplayer integrity. 
This feature is only available in single-player missions.
```

### UI Indicators
- Clear visual indication of MP vs SP mode
- Grayed-out blocked features in MP
- Tooltips explaining restrictions

## Technical Safeguards

### No Code Injection
- Export.lua is user-installed script
- No DLL injection into DCS process
- No modification of DCS binaries
- Uses only documented DCS APIs

### Local Communication Only
- Socket connection to 127.0.0.1 only
- No external network access
- No data transmission to external servers
- All processing happens locally

### Conservative Approach
- Unknown sessions treated as MP
- Fail-safe to blocked state
- Clear error messages
- Audit trail of blocked attempts

## Server Administrator Guidance

### What Flight Hub Does
- Reads basic flight telemetry (position, speed, altitude)
- Accesses engine data (RPM, temperature)
- Monitors aircraft configuration (gear, flaps)
- **Does NOT** access weapons, countermeasures, or tactical data in MP

### What Flight Hub Does NOT Do
- Provide tactical advantages
- Access restricted multiplayer data
- Modify DCS behavior
- Inject code into DCS process
- Communicate with external servers

### Verification
Server administrators can verify Flight Hub's behavior:

1. **Export.lua Inspection**: User-installed script is readable
2. **Network Monitoring**: Only local socket connections
3. **DCS Logs**: No injection or modification attempts
4. **Feature Testing**: Blocked features return no data in MP

## Compliance

### DCS Terms of Service
- Uses only documented APIs
- No reverse engineering
- No modification of game files
- Respects multiplayer policies

### Fair Play
- No tactical data in competitive environments
- Maintains level playing field
- Transparent about capabilities
- User-controlled installation

## Future Considerations

### Server-Specific Policies
- Potential for server-specific feature sets
- Whitelist/blacklist capabilities
- Server administrator controls

### Enhanced Detection
- More sophisticated MP detection
- Server policy negotiation
- Dynamic feature adjustment

### Community Standards
- Work with DCS community
- Respect server administrator wishes
- Maintain competitive integrity

## Contact

For questions about MP integrity or to report concerns:
- GitHub Issues: [Flight Hub Repository]
- Email: security@flight-hub.dev
- Discord: Flight Hub Community

## Changelog

### Version 1.0
- Initial MP integrity implementation
- Basic session detection
- Feature blocking system
- User documentation

---

*This document is maintained as part of Flight Hub's commitment to fair play and multiplayer integrity in the DCS community.*