# ADR-006: Driver-Light Integration Approach

## Status
Accepted

## Context

Flight simulators have varying integration capabilities and policies. Some support rich APIs (SimConnect), others require file modification (DCS Export.lua), and some have anti-cheat concerns. Traditional approaches either require kernel drivers, DLL injection, or extensive system modification, creating maintenance burden and compatibility issues.

## Decision

We adopt a "Driver-Light" approach that minimizes system integration footprint:

1. **No Kernel Drivers**: Use existing OS HID/USB infrastructure
2. **No DLL Injection**: Avoid injecting code into simulator processes
3. **Minimal File Changes**: Use simulator-provided integration points
4. **Reversible Integration**: All changes must be easily undoable
5. **User-Controlled**: Users install integration components themselves

### Integration Strategy by Simulator

#### MSFS (Microsoft Flight Simulator)
- **Method**: SimConnect API + Input Events
- **Footprint**: No file changes, API-only integration
- **Reversibility**: Automatic (no persistent changes)

#### X-Plane
- **Method**: UDP DataRefs + Optional Plugin
- **Footprint**: Optional plugin in Resources/plugins/
- **Reversibility**: Delete plugin folder

#### DCS (Digital Combat Simulator)
- **Method**: User-installed Export.lua in Saved Games
- **Footprint**: Single Lua file in user directory
- **Reversibility**: Delete or rename Export.lua
- **MP Safety**: Blocks write operations in multiplayer

### File System Impact Documentation

Each integration documents exactly what it touches:

```markdown
## MSFS Integration
- **Files Modified**: None
- **Registry Changes**: None
- **Network Ports**: None (SimConnect uses named pipes)
- **Revert Steps**: Stop Flight Hub service

## X-Plane Integration  
- **Files Modified**: Resources/plugins/FlightHub/ (optional)
- **Registry Changes**: None
- **Network Ports**: UDP 49000 (configurable)
- **Revert Steps**: Delete plugin folder, restart X-Plane

## DCS Integration
- **Files Modified**: Saved Games/DCS/Scripts/Export.lua
- **Registry Changes**: None  
- **Network Ports**: TCP 12345 (configurable)
- **Revert Steps**: Delete or rename Export.lua
```

## Consequences

### Positive
- Minimal system impact reduces support burden
- Easy installation and removal builds user confidence
- Compatibility with anti-cheat and security software
- Clear audit trail of system changes
- Reduced maintenance across simulator updates

### Negative
- Some advanced features may not be available
- Performance may be lower than kernel-level integration
- Dependency on simulator-provided APIs
- Limited control over timing and priority

## Alternatives Considered

1. **Kernel Drivers**: Rejected due to complexity, signing requirements, and user resistance
2. **DLL Injection**: Rejected due to anti-cheat conflicts and stability risks
3. **System Hooks**: Rejected due to security software conflicts
4. **Virtual Devices**: Considered for specific use cases but not as primary approach

## Implementation Guidelines

### File Modification Rules
- Only modify files in user-writable directories
- Always create backups before modification
- Provide clear revert instructions
- Document exact changes made

### Network Usage
- Use configurable ports with sane defaults
- Bind only to localhost by default
- Document all network activity
- Provide firewall configuration guidance

### Registry/System Changes
- Avoid registry modifications where possible
- Use user-level settings only (HKCU, not HKLM)
- Document all system changes
- Provide automated cleanup

### Integration Testing
- Test installation and removal procedures
- Verify clean uninstall leaves no traces
- Test compatibility with security software
- Validate multiplayer safety (DCS)

## Security Considerations

- No elevation required for normal operation
- All integration points use existing simulator APIs
- Network communication limited to localhost
- File changes limited to user directories
- Clear documentation of system impact

## User Experience

- Installation wizard explains what will be changed
- One-click revert for all integrations
- Clear status indicators for each simulator
- Troubleshooting guide for common issues

## References

- Flight Hub Requirements: LEG-01, SEC-01, XPLAT-01
- [Principle of Least Privilege](https://example.com)
- [Software Installation Best Practices](https://example.com)