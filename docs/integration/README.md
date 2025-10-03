# Flight Hub Integration Documentation

This directory contains comprehensive documentation about what Flight Hub touches in each supported flight simulator, including files, ports, variables, and how to revert changes.

## Simulator Integration Guides

- [Microsoft Flight Simulator (MSFS)](./msfs.md) - SimConnect integration and configuration changes
- [X-Plane](./xplane.md) - DataRef integration and preference modifications  
- [DCS World](./dcs.md) - Export.lua integration and multiplayer considerations

## Quick Reference

### Files Modified by Flight Hub

| Simulator | Files | Purpose | Revert Method |
|-----------|-------|---------|---------------|
| MSFS | `UserCfg.opt` | Disable built-in curves | [MSFS Revert](./msfs.md) |
| X-Plane | `X-Plane Joystick Settings.prf` | Linear curve settings | [X-Plane Revert](./xplane.md) |
| DCS | `options.lua` (user-installed) | Control curve settings | [DCS Revert](./dcs.md) |

### Network Ports Used

| Port | Protocol | Purpose | Simulator |
|------|----------|---------|-----------|
| Dynamic | TCP | SimConnect | MSFS |
| 49000 | UDP | DataRef communication | X-Plane |
| 12080 | TCP | Export.lua socket | DCS |

### Installation Requirements

- **Windows**: No administrator privileges required at runtime
- **Linux**: No root privileges required at runtime  
- **All Platforms**: User-level installation and operation

## Support and Troubleshooting

If you need to completely remove Flight Hub's changes:

1. Follow the revert steps in the appropriate simulator guide
2. Restart the simulator to ensure changes take effect
3. Verify functionality using the simulator's built-in control testing

For additional support, see our [troubleshooting guide](../troubleshooting.md) or contact support.