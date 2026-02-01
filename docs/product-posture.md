# Product Posture

## What Flight Hub Is

**Flight Hub is an accessory/input manager that requires MSFS, X-Plane, or DCS; it does not emulate or replace any simulator.**

Flight Hub is a peripheral management and input processing tool designed to enhance your flight simulation experience. It provides:

- **Input device management** - Unified control of joysticks, throttles, pedals, and other flight peripherals
- **Force feedback processing** - Safe, real-time FFB synthesis for compatible devices
- **Profile management** - Automatic profile switching based on detected aircraft
- **Panel integration** - Support for hardware panels and StreamDeck devices

## What Flight Hub Is Not

Flight Hub is **not**:

- A flight simulator
- A simulator replacement or emulator
- A tool for bypassing simulator licensing
- A network service or online platform

Flight Hub requires a licensed copy of one or more supported simulators to function. It operates as a local accessory application that communicates with simulators through their official APIs.

## Supported Simulators

Flight Hub integrates with the following simulators through their official APIs:

| Simulator | Integration Method | Required License |
|-----------|-------------------|------------------|
| Microsoft Flight Simulator (MSFS) | SimConnect API | MSFS license required |
| X-Plane | UDP protocol / Plugin | X-Plane license required |
| DCS World | Export.lua scripting | DCS World (free or modules) |

## Export Control Notice

Flight Hub is civilian software intended for entertainment and training purposes with consumer flight simulation software. Users are responsible for ensuring their use of Flight Hub complies with all applicable export control laws and regulations in their jurisdiction.

**Important considerations:**

- Flight Hub does not contain controlled technology or encryption beyond standard TLS for optional update checks
- Flight Hub does not interface with real aircraft systems or certified avionics
- Users in certain jurisdictions may have restrictions on simulation software - consult local regulations

## Simulator EULA Compliance

When using Flight Hub with supported simulators, you must comply with each simulator's End User License Agreement (EULA):

### Microsoft Flight Simulator

- Flight Hub uses the official SimConnect SDK as documented by Microsoft
- Users must own a valid MSFS license
- Flight Hub does not modify MSFS game files or bypass any licensing mechanisms
- See: [Microsoft Flight Simulator EULA](https://www.xbox.com/legal/slt)

### X-Plane

- Flight Hub uses X-Plane's documented UDP protocol and plugin interface
- Users must own a valid X-Plane license
- Flight Hub plugins are installed in the standard X-Plane plugins directory
- See: [X-Plane EULA](https://www.x-plane.com/kb/x-plane-eula/)

### DCS World

- Flight Hub uses DCS's documented Export.lua scripting interface
- Users must accept the DCS World EULA
- Flight Hub modifications to Export.lua are reversible and documented
- See: [DCS World EULA](https://www.digitalcombatsimulator.com/en/support/eula/)

## Data and Privacy

Flight Hub operates entirely locally on your computer:

- **No telemetry** - Flight Hub does not collect or transmit usage data
- **No accounts** - No user accounts or online services required
- **Local storage only** - All configuration and logs stored locally
- **Optional updates** - Update checks can be disabled; no data sent when disabled

## Third-Party Components

Flight Hub includes open-source components. A complete inventory of third-party components and their licenses is available in `third-party-components.toml` included with the installation.

## Questions?

For questions about Flight Hub's product posture or compliance:

- Review the [What We Touch](integration/) documentation for details on simulator integration
- Check the [FAQ](faq.md) for common questions
- Open an issue on the [GitHub repository](https://github.com/EffortlessMetrics/OpenFlight)

---

*This document was last updated for Flight Hub v1.0. For the latest version, see the [online documentation](https://github.com/EffortlessMetrics/OpenFlight).*
