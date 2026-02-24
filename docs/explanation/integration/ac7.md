# Ace Combat 7 Integration (Experimental)

This document explains what Flight Hub touches for Ace Combat 7 support, and what remains user-provided.

## Overview

Ace Combat 7 has no official telemetry SDK or plugin API comparable to SimConnect/X-Plane/DCS exports.  
Flight Hub therefore supports AC7 in an experimental mode that relies on a user-provided telemetry bridge (for example, UE4SS/Lua output over localhost UDP).

Flight Hub does not inject into AC7 and does not ship AC7 game code.

## Files Modified

### Input.ini (Managed Block)
**Location**: `%LOCALAPPDATA%\BANDAI NAMCO Entertainment\ACE COMBAT 7\Config\WindowsNoEditor\Input.ini`

**Purpose**: Install a Flight Hub managed input mapping block for HOTAS/joystick setups.

**Change Type**: Append/replace a delimited managed block; preserve user content outside the block.

### Input.ini Backup
**Location**: `%LOCALAPPDATA%\BANDAI NAMCO Entertainment\ACE COMBAT 7\Config\WindowsNoEditor\Input.ini.flight-hub.bak`

**Purpose**: Backup before managed edits.

### SaveGames (Read Only)
**Location**: `%LOCALAPPDATA%\BANDAI NAMCO Entertainment\ACE COMBAT 7\SaveGames`

**Purpose**: Support diagnostics and user guidance only. Flight Hub does not write save data.

## Network Connections

- **Port**: `7779/UDP` (default)
- **Direction**: Inbound localhost
- **Purpose**: Receive telemetry from user bridge plugin/process

## Revert Steps

1. Restore `Input.ini` from `Input.ini.flight-hub.bak`, or remove the Flight Hub managed block markers and enclosed lines.
2. Remove/disable your user-installed telemetry bridge plugin if no longer needed.
3. Restart AC7.

## Legal and EULA Notes

- AC7 integration assumes a legitimate, user-owned copy of the game.
- Flight Hub does not bypass DRM, licensing checks, or anti-tamper.
- Third-party bridge plugins may have legal/EULA implications independent of Flight Hub.
- Use bridge tooling at your own risk and verify publisher/server policies before multiplayer usage.

## What Flight Hub Does NOT Touch

- AC7 executable binaries.
- DRM/anti-tamper components.
- Multiplayer services, accounts, or server state.
- Proprietary AC7 assets distributed by the game.
