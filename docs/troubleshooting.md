---
doc_id: DOC-TROUBLESHOOTING
kind: how-to
area: infra
status: active
links:
  requirements: []
  tasks: []
  adrs: []
---

# Flight Hub Troubleshooting Guide

This guide helps resolve common issues with Flight Hub installation, configuration, and simulator integration.

## Installation Issues

### Windows Installation Problems

**Issue**: MSI installer fails with permission errors
**Solution**: 
1. Ensure you're installing to user directory (not Program Files)
2. Run installer as current user (not administrator)
3. Check Windows Defender hasn't quarantined the installer

**Issue**: "Application failed to start" after installation
**Solution**:
1. Install Microsoft Visual C++ Redistributable
2. Check Windows Event Viewer for specific error details
3. Try running in Safe Mode (see below)

### Linux Installation Problems

**Issue**: Systemd service fails to start
**Solution**:
1. Check service status: `systemctl --user status flight-hub`
2. Verify executable permissions: `ls -la /usr/local/bin/flight-hub`
3. Check logs: `journalctl --user -u flight-hub`

**Issue**: Permission denied errors
**Solution**:
1. Ensure installation was done without sudo
2. Check user has access to required directories
3. Verify udev rules are installed correctly

## Simulator Integration Issues

### Microsoft Flight Simulator (MSFS)

**Issue**: Flight Hub cannot connect to MSFS
**Solutions**:
1. Ensure MSFS is fully loaded (not just main menu)
2. Check SimConnect is enabled in MSFS settings
3. Verify Windows Firewall isn't blocking connections
4. Restart both MSFS and Flight Hub

**Issue**: Control inputs not working in MSFS
**Solutions**:
1. Check Flight Hub has disabled MSFS curves (UserCfg.opt)
2. Verify axis assignments in Flight Hub match MSFS
3. Test with default aircraft first
4. Check for conflicting input software

**Issue**: MSFS crashes when Flight Hub connects
**Solutions**:
1. Update MSFS to latest version
2. Disable other SimConnect applications temporarily
3. Check MSFS Community folder for conflicting addons
4. Try Flight Hub Safe Mode

### X-Plane

**Issue**: No data received from X-Plane
**Solutions**:
1. Verify UDP port 49000 is not blocked
2. Check X-Plane network settings allow UDP
3. Ensure DataRef output is enabled
4. Try different UDP port in Flight Hub settings

**Issue**: X-Plane plugin not loading
**Solutions**:
1. Check plugin is in correct directory: `X-Plane/Resources/plugins/FlightHub/`
2. Verify plugin file matches X-Plane version (32/64-bit)
3. Check X-Plane Log.txt for plugin errors
4. Ensure plugin dependencies are met

**Issue**: Controls feel different after Flight Hub installation
**Solutions**:
1. Check X-Plane response curves are disabled
2. Adjust Flight Hub curve settings
3. Verify axis calibration in Flight Hub
4. Compare with X-Plane's built-in curves temporarily

### DCS World

**Issue**: Export.lua not working
**Solutions**:
1. Verify Export.lua is in correct location: `Saved Games/DCS/Scripts/`
2. Check DCS scripting is enabled
3. Review DCS.log for Lua errors
4. Ensure Export.lua syntax is correct

**Issue**: No data in multiplayer
**Solutions**:
1. This is expected - many features are blocked in MP for integrity
2. Check Flight Hub UI shows "Multiplayer Session" banner
3. Verify single-player mode works correctly
4. Review multiplayer restrictions in documentation

**Issue**: DCS performance impact
**Solutions**:
1. Reduce Export.lua update rate
2. Disable unnecessary telemetry features
3. Check for other Export.lua conflicts
4. Monitor DCS frame rate and adjust accordingly

## Performance Issues

### High CPU Usage

**Symptoms**: Flight Hub using >5% CPU constantly
**Solutions**:
1. Check for runaway processes in Task Manager
2. Reduce update rates in settings
3. Disable unnecessary features (panels, tactile)
4. Update to latest Flight Hub version

### High Memory Usage

**Symptoms**: Flight Hub using >200MB RAM
**Solutions**:
1. Restart Flight Hub service
2. Check for memory leaks in logs
3. Reduce blackbox recording duration
4. Disable debug logging if enabled

### Input Lag or Jitter

**Symptoms**: Noticeable delay or inconsistency in controls
**Solutions**:
1. Check system meets minimum requirements
2. Close unnecessary background applications
3. Verify USB devices are on dedicated controllers
4. Try different USB ports (avoid hubs)
5. Check Windows power management settings

## Network and Connectivity Issues

### Firewall Problems

**Issue**: Windows Firewall blocking connections
**Solutions**:
1. Add Flight Hub to Windows Firewall exceptions
2. Allow Flight Hub through private networks
3. Check third-party firewall software
4. Temporarily disable firewall for testing

### Port Conflicts

**Issue**: "Port already in use" errors
**Solutions**:
1. Check what's using the port: `netstat -an | findstr :49000`
2. Change port in Flight Hub settings
3. Restart conflicting applications
4. Reboot system if necessary

### Antivirus Interference

**Issue**: Antivirus blocking Flight Hub
**Solutions**:
1. Add Flight Hub directory to antivirus exclusions
2. Whitelist Flight Hub executable
3. Check quarantine for Flight Hub files
4. Temporarily disable real-time protection for testing

## Configuration Issues

### Profile Problems

**Issue**: Profiles not loading or applying
**Solutions**:
1. Check profile JSON syntax is valid
2. Verify profile file permissions
3. Review Flight Hub logs for profile errors
4. Try with default profile first

**Issue**: Aircraft auto-detection not working
**Solutions**:
1. Verify simulator is properly detected
2. Check aircraft identification in logs
3. Ensure profile naming matches aircraft
4. Test manual profile selection

### Calibration Issues

**Issue**: Axis calibration seems wrong
**Solutions**:
1. Recalibrate in Flight Hub settings
2. Check for hardware issues (potentiometer wear)
3. Verify axis assignment is correct
4. Test with different USB port

## Safe Mode and Recovery

### Starting in Safe Mode

When Flight Hub has issues, start in Safe Mode:

**Windows**: 
```cmd
flight-hub.exe --safe
```

**Linux**:
```bash
flight-hub --safe
```

Safe Mode disables:
- Panel integrations
- Plugin system
- Tactile feedback
- Advanced features

Only basic axis processing remains active.

### Complete Reset

To completely reset Flight Hub configuration:

1. **Stop Flight Hub service**
2. **Backup important profiles** (optional)
3. **Delete configuration directory**:
   - Windows: `%APPDATA%\FlightHub`
   - Linux: `~/.config/flight-hub`
4. **Restart Flight Hub**
5. **Run first-time setup wizard**

### Reverting Simulator Changes

If you need to completely remove Flight Hub's simulator integration:

- **MSFS**: [Follow MSFS revert steps](./integration/msfs.md#revert-steps)
- **X-Plane**: [Follow X-Plane revert steps](./integration/xplane.md#revert-steps)  
- **DCS**: [Follow DCS revert steps](./integration/dcs.md#revert-steps)

## Diagnostic Information

### Collecting Logs

Flight Hub logs are located:
- **Windows**: `%APPDATA%\FlightHub\logs\`
- **Linux**: `~/.local/share/flight-hub/logs/`

Important log files:
- `flight-hub.log` - Main application log
- `axis-engine.log` - Real-time axis processing
- `integration.log` - Simulator integration events

### System Information

When reporting issues, include:
1. Flight Hub version
2. Operating system and version
3. Simulator version(s)
4. Hardware configuration (controllers, etc.)
5. Relevant log excerpts
6. Steps to reproduce the issue

### Performance Monitoring

Check Flight Hub performance:
1. Open Task Manager / System Monitor
2. Monitor Flight Hub CPU and memory usage
3. Check for consistent high usage patterns
4. Note any correlation with simulator events

## Getting Help

### Self-Service Resources

1. **Documentation**: Check the integration guides for your simulator
2. **FAQ**: Review common questions and answers
3. **Community Forums**: Search existing discussions
4. **GitHub Issues**: Check for known issues and workarounds

### Contacting Support

When contacting support, provide:
1. **Clear problem description**
2. **Steps to reproduce**
3. **System information**
4. **Log files** (recent entries)
5. **Screenshots** (if UI-related)

### Emergency Recovery

If Flight Hub is completely broken:
1. **Uninstall Flight Hub**
2. **Follow simulator revert steps**
3. **Clean install Flight Hub**
4. **Restore backed-up profiles**
5. **Reconfigure step-by-step**

---

**Last Updated**: Based on Flight Hub specification and common user issues
**Version**: 1.0 - Initial troubleshooting guide