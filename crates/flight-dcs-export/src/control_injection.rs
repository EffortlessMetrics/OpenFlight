// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS control command injection
//!
//! Serialises axis and button commands into the DCS Export.lua protocol
//! format for sending back to DCS via UDP. Commands are buffered and
//! flushed once per frame to avoid flooding the socket.
//!
//! Also provides device/command ID tables for common DCS modules and
//! a [`DcsUdpSender`] that wraps a UDP socket targeting the DCS command
//! port (default 7778).

use std::net::{SocketAddr, UdpSocket};

/// Default DCS command receive port.
pub const DCS_DEFAULT_COMMAND_PORT: u16 = 7778;

/// Type of control action to send to DCS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DcsActionType {
    /// Continuous axis value in `[-1.0, 1.0]`.
    Axis,
    /// Momentary button press (value `1.0` for press, `0.0` for release).
    ButtonPress,
    /// Button release.
    ButtonRelease,
    /// Toggle switch — sends value `1.0` to toggle, DCS handles the state.
    Toggle,
}

/// A single DCS input command targeting a cockpit device.
///
/// In DCS, devices are identified by numeric IDs (e.g. device 0 is the
/// main flight controls). Each device exposes numbered command slots.
#[derive(Debug, Clone, PartialEq)]
pub struct DcsControlCommand {
    /// DCS device ID (e.g. 0 = flight controls).
    pub device_id: u32,
    /// Command ID within the device.
    pub command_id: u32,
    /// Command value, typically in the range `[-1.0, 1.0]` for axes or
    /// `0.0`/`1.0` for buttons.
    pub value: f64,
    /// Type of control action.
    pub action_type: DcsActionType,
}

impl DcsControlCommand {
    fn new(device_id: u32, command_id: u32, value: f64, action_type: DcsActionType) -> Self {
        Self {
            device_id,
            command_id,
            value,
            action_type,
        }
    }

    /// Create an axis command.
    pub fn axis(device_id: u32, command_id: u32, value: f64) -> Self {
        Self::new(
            device_id,
            command_id,
            value.clamp(-1.0, 1.0),
            DcsActionType::Axis,
        )
    }

    /// Create a button press command.
    pub fn button_press(device_id: u32, command_id: u32) -> Self {
        Self::new(device_id, command_id, 1.0, DcsActionType::ButtonPress)
    }

    /// Create a button release command.
    pub fn button_release(device_id: u32, command_id: u32) -> Self {
        Self::new(device_id, command_id, 0.0, DcsActionType::ButtonRelease)
    }

    /// Create a toggle command.
    pub fn toggle(device_id: u32, command_id: u32) -> Self {
        Self::new(device_id, command_id, 1.0, DcsActionType::Toggle)
    }

    /// Serialize this command to the DCS wire-format line (without trailing newline).
    pub fn to_wire(&self) -> String {
        let prefix = match self.action_type {
            DcsActionType::Axis => "CMD",
            DcsActionType::ButtonPress | DcsActionType::ButtonRelease => "BTN",
            DcsActionType::Toggle => "TGL",
        };
        format!(
            "{}:{},{},{:.6}",
            prefix, self.device_id, self.command_id, self.value
        )
    }
}

/// Buffers and serialises control commands for DCS.
///
/// Commands are queued during a processing tick and flushed as a single
/// UDP payload at the end of the frame.
#[derive(Debug)]
pub struct DcsControlInjector {
    buffer: Vec<DcsControlCommand>,
    max_commands_per_frame: usize,
}

impl DcsControlInjector {
    /// Create a new injector with the given per-frame command limit.
    pub fn new(max_commands_per_frame: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(max_commands_per_frame),
            max_commands_per_frame,
        }
    }

    /// Queue a command for the next flush.
    ///
    /// Returns `false` if the buffer is full (command is dropped).
    pub fn queue_command(&mut self, cmd: DcsControlCommand) -> bool {
        if self.buffer.len() >= self.max_commands_per_frame {
            return false;
        }
        self.buffer.push(cmd);
        true
    }

    /// Convenience: queue a button press for `(device_id, command_id)`.
    pub fn press_button(&mut self, device_id: u32, command_id: u32) -> bool {
        self.queue_command(DcsControlCommand::button_press(device_id, command_id))
    }

    /// Convenience: queue a button release for `(device_id, command_id)`.
    pub fn release_button(&mut self, device_id: u32, command_id: u32) -> bool {
        self.queue_command(DcsControlCommand::button_release(device_id, command_id))
    }

    /// Convenience: queue an axis command by name.
    ///
    /// Looks up the named axis in the well-known axis table and queues the
    /// appropriate device/command pair. Returns `false` if the axis name is
    /// unknown or the buffer is full.
    pub fn set_axis(&mut self, axis_name: &str, value: f64) -> bool {
        if let Some(&(device_id, command_id)) = lookup_axis(axis_name) {
            self.queue_command(DcsControlCommand::axis(device_id, command_id, value))
        } else {
            false
        }
    }

    /// Number of commands waiting to be flushed.
    pub fn pending_count(&self) -> usize {
        self.buffer.len()
    }

    /// Discard all pending commands without sending.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Serialise all pending commands into a DCS-protocol UDP payload and
    /// drain the buffer.
    ///
    /// The wire format is newline-separated entries:
    /// ```text
    /// CMD:<device_id>,<command_id>,<value>\n
    /// BTN:<device_id>,<command_id>,<value>\n
    /// TGL:<device_id>,<command_id>,<value>\n
    /// ```
    ///
    /// Values are formatted with up to 6 decimal places.
    pub fn flush(&mut self) -> Vec<u8> {
        if self.buffer.is_empty() {
            return Vec::new();
        }

        let mut out = String::with_capacity(self.buffer.len() * 32);
        for cmd in self.buffer.drain(..) {
            out.push_str(&cmd.to_wire());
            out.push('\n');
        }
        out.into_bytes()
    }

    /// Maximum commands allowed per frame.
    pub fn max_commands_per_frame(&self) -> usize {
        self.max_commands_per_frame
    }
}

impl Default for DcsControlInjector {
    fn default() -> Self {
        Self::new(64)
    }
}

// ---------------------------------------------------------------------------
// DcsUdpSender — sends flushed payloads to DCS over UDP
// ---------------------------------------------------------------------------

/// Sends command payloads to DCS over a UDP socket.
///
/// Wraps a `UdpSocket` bound to an ephemeral local port and targeting the
/// DCS command port (default `127.0.0.1:7778`).
#[derive(Debug)]
pub struct DcsUdpSender {
    socket: UdpSocket,
    target: SocketAddr,
}

impl DcsUdpSender {
    /// Create a sender targeting the given DCS address.
    pub fn new(target: SocketAddr) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        Ok(Self { socket, target })
    }

    /// Create a sender targeting `127.0.0.1:<port>`.
    pub fn localhost(port: u16) -> std::io::Result<Self> {
        let target: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
        Self::new(target)
    }

    /// Create a sender using the default DCS command port.
    pub fn default_port() -> std::io::Result<Self> {
        Self::localhost(DCS_DEFAULT_COMMAND_PORT)
    }

    /// Send a raw payload to DCS. Returns number of bytes sent.
    pub fn send(&self, payload: &[u8]) -> std::io::Result<usize> {
        self.socket.send_to(payload, self.target)
    }

    /// Flush an injector and send the resulting payload.
    ///
    /// Returns `Ok(0)` if the injector was empty.
    pub fn flush_and_send(&self, injector: &mut DcsControlInjector) -> std::io::Result<usize> {
        let payload = injector.flush();
        if payload.is_empty() {
            return Ok(0);
        }
        self.send(&payload)
    }

    /// Target address.
    pub fn target(&self) -> SocketAddr {
        self.target
    }
}

// ---------------------------------------------------------------------------
// Device / command ID tables for common DCS modules
// ---------------------------------------------------------------------------

/// A DCS device definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DcsDevice {
    pub id: u32,
    pub name: &'static str,
}

impl DcsDevice {
    /// Create a DCS device definition.
    pub const fn new(id: u32, name: &'static str) -> Self {
        Self { id, name }
    }
}

/// A clickable cockpit command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DcsCommandDef {
    pub device_id: u32,
    pub command_id: u32,
    pub name: &'static str,
}

impl DcsCommandDef {
    /// Create a clickable cockpit command definition.
    pub const fn new(device_id: u32, command_id: u32, name: &'static str) -> Self {
        Self {
            device_id,
            command_id,
            name,
        }
    }
}

fn lookup_command_in(
    commands: &'static [DcsCommandDef],
    name: &str,
) -> Option<&'static DcsCommandDef> {
    commands.iter().find(|c| c.name == name)
}

/// Well-known named axes shared across modules.
///
/// Each entry maps an axis name to `(device_id, command_id)`.
static COMMON_AXES: &[(&str, (u32, u32))] = &[
    ("pitch", (0, 2001)),
    ("roll", (0, 2002)),
    ("yaw", (0, 2003)),
    ("throttle", (0, 2004)),
    ("throttle_left", (0, 2005)),
    ("throttle_right", (0, 2006)),
    ("wheel_brake_left", (0, 2007)),
    ("wheel_brake_right", (0, 2008)),
    ("nosewheel_steering", (0, 2009)),
];

/// Look up an axis name → `(device_id, command_id)`.
pub fn lookup_axis(name: &str) -> Option<&'static (u32, u32)> {
    let lower = name.to_ascii_lowercase();
    COMMON_AXES
        .iter()
        .find(|(n, _)| *n == lower)
        .map(|(_, ids)| ids)
}

/// Return all well-known axis names.
pub fn all_axis_names() -> Vec<&'static str> {
    COMMON_AXES.iter().map(|(n, _)| *n).collect()
}

// --- F/A-18C Hornet ----------------------------------------------------------

/// F/A-18C device IDs.
pub mod fa18c {
    use super::{DcsCommandDef, DcsDevice, lookup_command_in};

    pub const UFC: DcsDevice = DcsDevice::new(25, "UFC");
    pub const HOTAS: DcsDevice = DcsDevice::new(12, "HOTAS");
    pub const IFEI: DcsDevice = DcsDevice::new(36, "IFEI");
    pub const LEFT_DDI: DcsDevice = DcsDevice::new(35, "Left DDI");
    pub const RIGHT_DDI: DcsDevice = DcsDevice::new(37, "Right DDI");

    pub static COMMANDS: &[DcsCommandDef] = &[
        DcsCommandDef::new(25, 3001, "UFC_1"),
        DcsCommandDef::new(25, 3002, "UFC_2"),
        DcsCommandDef::new(25, 3003, "UFC_3"),
        DcsCommandDef::new(25, 3004, "UFC_4"),
        DcsCommandDef::new(25, 3005, "UFC_5"),
        DcsCommandDef::new(25, 3006, "UFC_6"),
        DcsCommandDef::new(25, 3007, "UFC_7"),
        DcsCommandDef::new(25, 3008, "UFC_8"),
        DcsCommandDef::new(25, 3009, "UFC_9"),
        DcsCommandDef::new(25, 3010, "UFC_0"),
        DcsCommandDef::new(25, 3018, "UFC_ENT"),
        DcsCommandDef::new(25, 3019, "UFC_CLR"),
        DcsCommandDef::new(12, 3200, "MASTER_ARM_ON"),
        DcsCommandDef::new(12, 3201, "MASTER_ARM_OFF"),
    ];

    pub fn lookup_command(name: &str) -> Option<&'static DcsCommandDef> {
        lookup_command_in(COMMANDS, name)
    }
}

// --- F-16C Viper -------------------------------------------------------------

pub mod f16c {
    use super::{DcsCommandDef, DcsDevice, lookup_command_in};

    pub const ICP: DcsDevice = DcsDevice::new(17, "ICP");
    pub const UFC: DcsDevice = DcsDevice::new(17, "UFC");
    pub const HOTAS: DcsDevice = DcsDevice::new(12, "HOTAS");

    pub static COMMANDS: &[DcsCommandDef] = &[
        DcsCommandDef::new(17, 3001, "ICP_0"),
        DcsCommandDef::new(17, 3002, "ICP_1"),
        DcsCommandDef::new(17, 3003, "ICP_2"),
        DcsCommandDef::new(17, 3004, "ICP_3"),
        DcsCommandDef::new(17, 3005, "ICP_4"),
        DcsCommandDef::new(17, 3006, "ICP_5"),
        DcsCommandDef::new(17, 3007, "ICP_6"),
        DcsCommandDef::new(17, 3008, "ICP_7"),
        DcsCommandDef::new(17, 3009, "ICP_8"),
        DcsCommandDef::new(17, 3010, "ICP_9"),
        DcsCommandDef::new(17, 3011, "ICP_ENTR"),
        DcsCommandDef::new(17, 3012, "ICP_RCL"),
        DcsCommandDef::new(17, 3015, "ICP_DCS_UP"),
        DcsCommandDef::new(17, 3016, "ICP_DCS_DOWN"),
        DcsCommandDef::new(12, 3100, "MASTER_ARM_TOGGLE"),
    ];

    pub fn lookup_command(name: &str) -> Option<&'static DcsCommandDef> {
        lookup_command_in(COMMANDS, name)
    }
}

// --- A-10C / A-10C II --------------------------------------------------------

pub mod a10c {
    use super::{DcsCommandDef, DcsDevice, lookup_command_in};

    pub const CDU: DcsDevice = DcsDevice::new(24, "CDU");
    pub const HOTAS: DcsDevice = DcsDevice::new(12, "HOTAS");
    pub const CMSP: DcsDevice = DcsDevice::new(39, "CMSP");

    pub static COMMANDS: &[DcsCommandDef] = &[
        DcsCommandDef::new(24, 3001, "CDU_1"),
        DcsCommandDef::new(24, 3002, "CDU_2"),
        DcsCommandDef::new(24, 3003, "CDU_3"),
        DcsCommandDef::new(24, 3004, "CDU_4"),
        DcsCommandDef::new(24, 3005, "CDU_5"),
        DcsCommandDef::new(24, 3006, "CDU_6"),
        DcsCommandDef::new(24, 3007, "CDU_7"),
        DcsCommandDef::new(24, 3008, "CDU_8"),
        DcsCommandDef::new(24, 3009, "CDU_9"),
        DcsCommandDef::new(24, 3010, "CDU_0"),
        DcsCommandDef::new(12, 3250, "MASTER_ARM_ON"),
        DcsCommandDef::new(12, 3251, "MASTER_ARM_OFF"),
        DcsCommandDef::new(39, 3001, "CMSP_JMR"),
        DcsCommandDef::new(39, 3002, "CMSP_MWS"),
    ];

    pub fn lookup_command(name: &str) -> Option<&'static DcsCommandDef> {
        lookup_command_in(COMMANDS, name)
    }
}

// --- F-14B Tomcat ------------------------------------------------------------

pub mod f14b {
    use super::{DcsCommandDef, DcsDevice, lookup_command_in};

    pub const PILOT_STICK: DcsDevice = DcsDevice::new(0, "Pilot Stick");
    pub const RIO_CAP: DcsDevice = DcsDevice::new(42, "RIO CAP");

    pub static COMMANDS: &[DcsCommandDef] = &[
        DcsCommandDef::new(0, 3014, "WING_SWEEP_AUTO"),
        DcsCommandDef::new(0, 3015, "WING_SWEEP_MANUAL"),
        DcsCommandDef::new(42, 3100, "RIO_CAP_TID_MODE"),
        DcsCommandDef::new(42, 3101, "RIO_CAP_LAUNCH"),
    ];

    pub fn lookup_command(name: &str) -> Option<&'static DcsCommandDef> {
        lookup_command_in(COMMANDS, name)
    }
}

// --- AH-64D Apache -----------------------------------------------------------

pub mod ah64d {
    use super::{DcsCommandDef, DcsDevice, lookup_command_in};

    pub const PILOT_KU: DcsDevice = DcsDevice::new(29, "Pilot KU");
    pub const CPG_KU: DcsDevice = DcsDevice::new(30, "CPG KU");
    pub const PILOT_TEDAC: DcsDevice = DcsDevice::new(28, "Pilot TEDAC");

    pub static COMMANDS: &[DcsCommandDef] = &[
        DcsCommandDef::new(29, 3001, "PLT_KU_A"),
        DcsCommandDef::new(29, 3026, "PLT_KU_ENT"),
        DcsCommandDef::new(29, 3027, "PLT_KU_CLR"),
        DcsCommandDef::new(30, 3001, "CPG_KU_A"),
        DcsCommandDef::new(30, 3026, "CPG_KU_ENT"),
        DcsCommandDef::new(30, 3027, "CPG_KU_CLR"),
    ];

    pub fn lookup_command(name: &str) -> Option<&'static DcsCommandDef> {
        lookup_command_in(COMMANDS, name)
    }
}

// ---------------------------------------------------------------------------
// Wire command parsing (deserialization)
// ---------------------------------------------------------------------------

/// Errors when parsing a wire-format command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireParseError {
    /// Missing or unrecognised prefix (CMD, BTN, TGL).
    UnknownPrefix(String),
    /// Incorrect number of fields after the prefix.
    BadFieldCount { expected: usize, got: usize },
    /// A numeric field could not be parsed.
    InvalidNumber(String),
}

impl std::fmt::Display for WireParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WireParseError::UnknownPrefix(p) => write!(f, "unknown prefix: {p}"),
            WireParseError::BadFieldCount { expected, got } => {
                write!(f, "expected {expected} fields, got {got}")
            }
            WireParseError::InvalidNumber(s) => write!(f, "invalid number: {s}"),
        }
    }
}

impl std::error::Error for WireParseError {}

/// Parse a single wire-format command line back into a [`DcsControlCommand`].
///
/// Accepted formats:
/// - `CMD:<device_id>,<command_id>,<value>`
/// - `BTN:<device_id>,<command_id>,<value>`
/// - `TGL:<device_id>,<command_id>,<value>`
pub fn parse_wire_command(line: &str) -> Result<DcsControlCommand, WireParseError> {
    let trimmed = line.trim();
    let (prefix, rest) = trimmed
        .split_once(':')
        .ok_or_else(|| WireParseError::UnknownPrefix(trimmed.to_string()))?;

    let parts: Vec<&str> = rest.split(',').collect();
    if parts.len() != 3 {
        return Err(WireParseError::BadFieldCount {
            expected: 3,
            got: parts.len(),
        });
    }

    let device_id: u32 = parts[0]
        .trim()
        .parse()
        .map_err(|_| WireParseError::InvalidNumber(parts[0].to_string()))?;
    let command_id: u32 = parts[1]
        .trim()
        .parse()
        .map_err(|_| WireParseError::InvalidNumber(parts[1].to_string()))?;
    let value: f64 = parts[2]
        .trim()
        .parse()
        .map_err(|_| WireParseError::InvalidNumber(parts[2].to_string()))?;

    let action_type = match prefix {
        "CMD" => DcsActionType::Axis,
        "BTN" => {
            if value > 0.5 {
                DcsActionType::ButtonPress
            } else {
                DcsActionType::ButtonRelease
            }
        }
        "TGL" => DcsActionType::Toggle,
        other => return Err(WireParseError::UnknownPrefix(other.to_string())),
    };

    Ok(DcsControlCommand {
        device_id,
        command_id,
        value,
        action_type,
    })
}

/// Parse a multi-line wire payload into a sequence of commands.
pub fn parse_wire_payload(payload: &str) -> Vec<Result<DcsControlCommand, WireParseError>> {
    payload
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(parse_wire_command)
        .collect()
}

// ---------------------------------------------------------------------------
// Clickable cockpit abstraction
// ---------------------------------------------------------------------------

/// A cockpit clickable control (switch, knob, button) identified by its
/// DCS device, button/arg number, and a human-readable label.
#[derive(Debug, Clone, PartialEq)]
pub struct Clickable {
    /// Human-readable label (e.g. "Master Arm Switch").
    pub label: &'static str,
    /// DCS device ID.
    pub device_id: u32,
    /// Button/argument number.
    pub button: u32,
    /// Minimum value (typically 0.0 for switches, -1.0 for 3-pos).
    pub min_value: f64,
    /// Maximum value (typically 1.0).
    pub max_value: f64,
}

impl Clickable {
    /// Create a [`DcsControlCommand`] to set this clickable to the given value.
    pub fn command(&self, value: f64) -> DcsControlCommand {
        let clamped = value.clamp(self.min_value, self.max_value);
        DcsControlCommand {
            device_id: self.device_id,
            command_id: self.button,
            value: clamped,
            action_type: if (self.max_value - self.min_value).abs() > 0.01 {
                DcsActionType::Axis
            } else {
                DcsActionType::Toggle
            },
        }
    }

    /// Create a press command (value = `max_value`).
    pub fn press(&self) -> DcsControlCommand {
        self.command(self.max_value)
    }

    /// Create a release command (value = `min_value`).
    pub fn release(&self) -> DcsControlCommand {
        self.command(self.min_value)
    }
}

// ---------------------------------------------------------------------------
// Per-aircraft axis mapping (bus axis name → DCS device arg)
// ---------------------------------------------------------------------------

/// Mapping from a Flight Hub bus axis name to a DCS device/command pair
/// for a specific aircraft module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AircraftAxisMapping {
    /// Flight Hub bus axis name (e.g. "pitch", "throttle_left").
    pub bus_axis: &'static str,
    /// DCS device ID.
    pub device_id: u32,
    /// DCS command/argument ID.
    pub command_id: u32,
}

impl AircraftAxisMapping {
    /// Create an aircraft-specific bus-axis mapping.
    pub const fn new(bus_axis: &'static str, device_id: u32, command_id: u32) -> Self {
        Self {
            bus_axis,
            device_id,
            command_id,
        }
    }
}

const PITCH_AXIS: AircraftAxisMapping = AircraftAxisMapping::new("pitch", 0, 2001);
const ROLL_AXIS: AircraftAxisMapping = AircraftAxisMapping::new("roll", 0, 2002);
const YAW_AXIS: AircraftAxisMapping = AircraftAxisMapping::new("yaw", 0, 2003);
const THROTTLE_AXIS: AircraftAxisMapping = AircraftAxisMapping::new("throttle", 0, 2004);
const THROTTLE_LEFT_AXIS: AircraftAxisMapping = AircraftAxisMapping::new("throttle_left", 0, 2005);
const THROTTLE_RIGHT_AXIS: AircraftAxisMapping =
    AircraftAxisMapping::new("throttle_right", 0, 2006);

/// F/A-18C Hornet axis mappings.
pub static FA18C_AXES: &[AircraftAxisMapping] = &[
    PITCH_AXIS,
    ROLL_AXIS,
    YAW_AXIS,
    THROTTLE_LEFT_AXIS,
    THROTTLE_RIGHT_AXIS,
];

/// F-16C Viper axis mappings.
pub static F16C_AXES: &[AircraftAxisMapping] = &[PITCH_AXIS, ROLL_AXIS, YAW_AXIS, THROTTLE_AXIS];

/// A-10C Warthog axis mappings.
pub static A10C_AXES: &[AircraftAxisMapping] = &[
    PITCH_AXIS,
    ROLL_AXIS,
    YAW_AXIS,
    THROTTLE_LEFT_AXIS,
    THROTTLE_RIGHT_AXIS,
];

/// Look up per-aircraft axis mapping by module name and bus axis.
pub fn lookup_aircraft_axis(module: &str, bus_axis: &str) -> Option<&'static AircraftAxisMapping> {
    let table: &[AircraftAxisMapping] = match module {
        "FA-18C_hornet" | "FA-18C" => FA18C_AXES,
        "F-16C_50" | "F-16C" => F16C_AXES,
        "A-10C" | "A-10C_2" => A10C_AXES,
        _ => return None,
    };
    table.iter().find(|m| m.bus_axis == bus_axis)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn axis_cmd(device: u32, cmd: u32, val: f64) -> DcsControlCommand {
        DcsControlCommand::axis(device, cmd, val)
    }

    #[test]
    fn test_new_injector_empty() {
        let inj = DcsControlInjector::new(16);
        assert_eq!(inj.pending_count(), 0);
    }

    #[test]
    fn test_queue_and_pending() {
        let mut inj = DcsControlInjector::new(16);
        assert!(inj.queue_command(axis_cmd(0, 1, 0.5)));
        assert!(inj.queue_command(axis_cmd(0, 2, -0.3)));
        assert_eq!(inj.pending_count(), 2);
    }

    #[test]
    fn test_queue_over_limit() {
        let mut inj = DcsControlInjector::new(2);
        assert!(inj.queue_command(axis_cmd(0, 1, 0.0)));
        assert!(inj.queue_command(axis_cmd(0, 2, 0.0)));
        assert!(!inj.queue_command(axis_cmd(0, 3, 0.0)));
        assert_eq!(inj.pending_count(), 2);
    }

    #[test]
    fn test_flush_produces_correct_format() {
        let mut inj = DcsControlInjector::new(16);
        inj.queue_command(axis_cmd(0, 10, 1.0));
        inj.queue_command(axis_cmd(3, 42, -0.5));

        let payload = inj.flush();
        let text = String::from_utf8(payload).unwrap();

        assert!(text.contains("CMD:0,10,1.000000\n"));
        assert!(text.contains("CMD:3,42,-0.500000\n"));
    }

    #[test]
    fn test_flush_drains_buffer() {
        let mut inj = DcsControlInjector::new(16);
        inj.queue_command(axis_cmd(0, 1, 0.0));
        assert_eq!(inj.pending_count(), 1);

        let _ = inj.flush();
        assert_eq!(inj.pending_count(), 0);
    }

    #[test]
    fn test_flush_empty_returns_empty() {
        let mut inj = DcsControlInjector::new(16);
        let payload = inj.flush();
        assert!(payload.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut inj = DcsControlInjector::new(16);
        inj.queue_command(axis_cmd(0, 1, 0.0));
        inj.queue_command(axis_cmd(0, 2, 0.0));
        inj.clear();
        assert_eq!(inj.pending_count(), 0);
    }

    #[test]
    fn test_default_max() {
        let inj = DcsControlInjector::default();
        assert_eq!(inj.max_commands_per_frame(), 64);
    }

    #[test]
    fn test_value_precision() {
        let mut inj = DcsControlInjector::new(16);
        inj.queue_command(axis_cmd(1, 1, std::f64::consts::PI / 4.0));
        let text = String::from_utf8(inj.flush()).unwrap();
        // Value is clamped to [-1, 1], so PI/4 ≈ 0.785398
        assert!(text.contains("0.785398"));
    }

    #[test]
    fn test_multiple_flushes() {
        let mut inj = DcsControlInjector::new(16);
        inj.queue_command(axis_cmd(0, 1, 0.1));
        let p1 = inj.flush();
        assert!(!p1.is_empty());

        inj.queue_command(axis_cmd(0, 2, 0.2));
        let p2 = inj.flush();
        assert!(!p2.is_empty());

        // First flush should not contain second command
        let t1 = String::from_utf8(p1).unwrap();
        assert!(!t1.contains("CMD:0,2,"));
    }

    // --- new command type tests ---

    #[test]
    fn test_button_press_command() {
        let cmd = DcsControlCommand::button_press(4, 3001);
        assert_eq!(cmd.device_id, 4);
        assert_eq!(cmd.command_id, 3001);
        assert!((cmd.value - 1.0).abs() < f64::EPSILON);
        assert_eq!(cmd.action_type, DcsActionType::ButtonPress);
    }

    #[test]
    fn test_button_release_command() {
        let cmd = DcsControlCommand::button_release(4, 3001);
        assert_eq!(cmd.device_id, 4);
        assert_eq!(cmd.command_id, 3001);
        assert!(cmd.value.abs() < f64::EPSILON);
        assert_eq!(cmd.action_type, DcsActionType::ButtonRelease);
    }

    #[test]
    fn test_toggle_command() {
        let cmd = DcsControlCommand::toggle(2, 500);
        assert_eq!(cmd.device_id, 2);
        assert_eq!(cmd.command_id, 500);
        assert!((cmd.value - 1.0).abs() < f64::EPSILON);
        assert_eq!(cmd.action_type, DcsActionType::Toggle);
    }

    #[test]
    fn test_axis_clamped() {
        let cmd = DcsControlCommand::axis(0, 1, 2.0);
        assert!((cmd.value - 1.0).abs() < f64::EPSILON);

        let cmd2 = DcsControlCommand::axis(0, 1, -2.0);
        assert!((cmd2.value - (-1.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_button_press_wire_format() {
        let mut inj = DcsControlInjector::new(16);
        inj.queue_command(DcsControlCommand::button_press(4, 3001));
        let text = String::from_utf8(inj.flush()).unwrap();
        assert!(text.contains("BTN:4,3001,1.000000\n"));
    }

    #[test]
    fn test_button_release_wire_format() {
        let mut inj = DcsControlInjector::new(16);
        inj.queue_command(DcsControlCommand::button_release(4, 3001));
        let text = String::from_utf8(inj.flush()).unwrap();
        assert!(text.contains("BTN:4,3001,0.000000\n"));
    }

    #[test]
    fn test_toggle_wire_format() {
        let mut inj = DcsControlInjector::new(16);
        inj.queue_command(DcsControlCommand::toggle(2, 500));
        let text = String::from_utf8(inj.flush()).unwrap();
        assert!(text.contains("TGL:2,500,1.000000\n"));
    }

    #[test]
    fn test_mixed_command_types() {
        let mut inj = DcsControlInjector::new(16);
        inj.queue_command(DcsControlCommand::axis(0, 1, 0.5));
        inj.queue_command(DcsControlCommand::button_press(4, 3001));
        inj.queue_command(DcsControlCommand::toggle(2, 500));
        inj.queue_command(DcsControlCommand::button_release(4, 3001));

        let text = String::from_utf8(inj.flush()).unwrap();
        assert!(text.contains("CMD:0,1,"));
        assert!(text.contains("BTN:4,3001,1.000000"));
        assert!(text.contains("TGL:2,500,"));
        assert!(text.contains("BTN:4,3001,0.000000"));
    }

    #[test]
    fn test_axis_zero() {
        let cmd = DcsControlCommand::axis(0, 1, 0.0);
        assert!(cmd.value.abs() < f64::EPSILON);
        assert_eq!(cmd.action_type, DcsActionType::Axis);
    }

    #[test]
    fn test_axis_negative() {
        let cmd = DcsControlCommand::axis(0, 1, -0.75);
        assert!((cmd.value - (-0.75)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fill_buffer_then_clear_then_refill() {
        let mut inj = DcsControlInjector::new(3);
        assert!(inj.queue_command(axis_cmd(0, 1, 0.1)));
        assert!(inj.queue_command(axis_cmd(0, 2, 0.2)));
        assert!(inj.queue_command(axis_cmd(0, 3, 0.3)));
        assert!(!inj.queue_command(axis_cmd(0, 4, 0.4))); // full
        inj.clear();
        assert_eq!(inj.pending_count(), 0);
        assert!(inj.queue_command(axis_cmd(0, 5, 0.5))); // can add again
        assert_eq!(inj.pending_count(), 1);
    }

    // --- press_button / release_button / set_axis convenience tests ---

    #[test]
    fn test_press_button_queues_correctly() {
        let mut inj = DcsControlInjector::new(16);
        assert!(inj.press_button(25, 3001));
        assert_eq!(inj.pending_count(), 1);

        let text = String::from_utf8(inj.flush()).unwrap();
        assert!(text.contains("BTN:25,3001,1.000000"));
    }

    #[test]
    fn test_release_button_queues_correctly() {
        let mut inj = DcsControlInjector::new(16);
        assert!(inj.release_button(25, 3001));
        assert_eq!(inj.pending_count(), 1);

        let text = String::from_utf8(inj.flush()).unwrap();
        assert!(text.contains("BTN:25,3001,0.000000"));
    }

    #[test]
    fn test_press_release_sequence() {
        let mut inj = DcsControlInjector::new(16);
        assert!(inj.press_button(25, 3001));
        assert!(inj.release_button(25, 3001));
        assert_eq!(inj.pending_count(), 2);

        let text = String::from_utf8(inj.flush()).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "BTN:25,3001,1.000000");
        assert_eq!(lines[1], "BTN:25,3001,0.000000");
    }

    #[test]
    fn test_set_axis_known_name() {
        let mut inj = DcsControlInjector::new(16);
        assert!(inj.set_axis("pitch", 0.5));
        assert_eq!(inj.pending_count(), 1);

        let text = String::from_utf8(inj.flush()).unwrap();
        assert!(text.contains("CMD:0,2001,0.500000"));
    }

    #[test]
    fn test_set_axis_case_insensitive() {
        let mut inj = DcsControlInjector::new(16);
        assert!(inj.set_axis("Throttle", 1.0));
        let text = String::from_utf8(inj.flush()).unwrap();
        assert!(text.contains("CMD:0,2004,1.000000"));
    }

    #[test]
    fn test_set_axis_unknown_returns_false() {
        let mut inj = DcsControlInjector::new(16);
        assert!(!inj.set_axis("nonexistent_axis", 0.0));
        assert_eq!(inj.pending_count(), 0);
    }

    #[test]
    fn test_set_axis_clamps_value() {
        let mut inj = DcsControlInjector::new(16);
        assert!(inj.set_axis("roll", 5.0));
        let text = String::from_utf8(inj.flush()).unwrap();
        assert!(text.contains("CMD:0,2002,1.000000"));
    }

    #[test]
    fn test_all_common_axes_resolvable() {
        let names = all_axis_names();
        assert!(names.len() >= 9);
        for name in &names {
            assert!(
                lookup_axis(name).is_some(),
                "axis '{}' should resolve",
                name
            );
        }
    }

    #[test]
    fn test_axis_value_ranges() {
        // Minimum
        let cmd = DcsControlCommand::axis(0, 2001, -1.0);
        assert!((cmd.value - (-1.0)).abs() < f64::EPSILON);

        // Maximum
        let cmd = DcsControlCommand::axis(0, 2001, 1.0);
        assert!((cmd.value - 1.0).abs() < f64::EPSILON);

        // Center
        let cmd = DcsControlCommand::axis(0, 2001, 0.0);
        assert!(cmd.value.abs() < f64::EPSILON);

        // Precision
        let cmd = DcsControlCommand::axis(0, 2001, 0.123456);
        assert!((cmd.value - 0.123456).abs() < 1e-10);
    }

    // --- to_wire tests ---

    #[test]
    fn test_to_wire_axis() {
        let cmd = DcsControlCommand::axis(0, 2001, 0.5);
        assert_eq!(cmd.to_wire(), "CMD:0,2001,0.500000");
    }

    #[test]
    fn test_to_wire_button_press() {
        let cmd = DcsControlCommand::button_press(25, 3001);
        assert_eq!(cmd.to_wire(), "BTN:25,3001,1.000000");
    }

    #[test]
    fn test_to_wire_toggle() {
        let cmd = DcsControlCommand::toggle(2, 500);
        assert_eq!(cmd.to_wire(), "TGL:2,500,1.000000");
    }

    // --- Device/command table tests ---

    #[test]
    fn test_fa18c_ufc_device() {
        assert_eq!(fa18c::UFC.id, 25);
        assert_eq!(fa18c::UFC.name, "UFC");
    }

    #[test]
    fn test_fa18c_command_lookup() {
        let cmd = fa18c::lookup_command("UFC_1").unwrap();
        assert_eq!(cmd.device_id, 25);
        assert_eq!(cmd.command_id, 3001);
    }

    #[test]
    fn test_fa18c_command_lookup_missing() {
        assert!(fa18c::lookup_command("NONEXISTENT").is_none());
    }

    #[test]
    fn test_fa18c_master_arm() {
        let on = fa18c::lookup_command("MASTER_ARM_ON").unwrap();
        let off = fa18c::lookup_command("MASTER_ARM_OFF").unwrap();
        assert_eq!(on.device_id, 12);
        assert_eq!(off.device_id, 12);
        assert_ne!(on.command_id, off.command_id);
    }

    #[test]
    fn test_f16c_icp_commands() {
        let cmd = f16c::lookup_command("ICP_ENTR").unwrap();
        assert_eq!(cmd.device_id, 17);
        assert_eq!(cmd.command_id, 3011);
    }

    #[test]
    fn test_f16c_dcs_up_down() {
        let up = f16c::lookup_command("ICP_DCS_UP").unwrap();
        let down = f16c::lookup_command("ICP_DCS_DOWN").unwrap();
        assert_ne!(up.command_id, down.command_id);
    }

    #[test]
    fn test_a10c_cdu_commands() {
        let cmd = a10c::lookup_command("CDU_5").unwrap();
        assert_eq!(cmd.device_id, 24);
        assert_eq!(cmd.command_id, 3005);
    }

    #[test]
    fn test_a10c_cmsp_commands() {
        let jmr = a10c::lookup_command("CMSP_JMR").unwrap();
        assert_eq!(jmr.device_id, 39);
    }

    #[test]
    fn test_f14b_rio_commands() {
        let cmd = f14b::lookup_command("RIO_CAP_LAUNCH").unwrap();
        assert_eq!(cmd.device_id, 42);
    }

    #[test]
    fn test_ah64d_pilot_and_cpg_ku() {
        let plt = ah64d::lookup_command("PLT_KU_ENT").unwrap();
        let cpg = ah64d::lookup_command("CPG_KU_ENT").unwrap();
        assert_ne!(plt.device_id, cpg.device_id);
        assert_eq!(plt.command_id, cpg.command_id);
    }

    #[test]
    fn test_multi_module_command_tables_no_empty() {
        assert!(!fa18c::COMMANDS.is_empty());
        assert!(!f16c::COMMANDS.is_empty());
        assert!(!a10c::COMMANDS.is_empty());
        assert!(!f14b::COMMANDS.is_empty());
        assert!(!ah64d::COMMANDS.is_empty());
    }

    #[test]
    fn test_multi_module_all_commands_unique_per_module() {
        for commands in &[
            fa18c::COMMANDS,
            f16c::COMMANDS,
            a10c::COMMANDS,
            f14b::COMMANDS,
            ah64d::COMMANDS,
        ] {
            let names: Vec<&str> = commands.iter().map(|c| c.name).collect();
            let mut uniq = names.clone();
            uniq.sort();
            uniq.dedup();
            assert_eq!(names.len(), uniq.len(), "duplicate command names found");
        }
    }

    // --- DcsUdpSender tests (loopback) ---

    #[test]
    fn test_udp_sender_loopback() {
        // Bind a receiver on an ephemeral port
        let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
        receiver.set_nonblocking(true).unwrap();
        let recv_addr = receiver.local_addr().unwrap();

        let sender = DcsUdpSender::new(recv_addr).unwrap();
        assert_eq!(sender.target(), recv_addr);

        let mut inj = DcsControlInjector::new(16);
        inj.press_button(25, 3001);
        inj.release_button(25, 3001);

        let sent = sender.flush_and_send(&mut inj).unwrap();
        assert!(sent > 0);

        let mut buf = [0u8; 1024];
        let (n, _) = receiver.recv_from(&mut buf).unwrap();
        let received = String::from_utf8_lossy(&buf[..n]);
        assert!(received.contains("BTN:25,3001,1.000000"));
        assert!(received.contains("BTN:25,3001,0.000000"));
    }

    #[test]
    fn test_udp_sender_empty_flush() {
        let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
        let recv_addr = receiver.local_addr().unwrap();
        let sender = DcsUdpSender::new(recv_addr).unwrap();

        let mut inj = DcsControlInjector::new(16);
        let sent = sender.flush_and_send(&mut inj).unwrap();
        assert_eq!(sent, 0);
    }

    // --- Wire command parsing tests ---

    #[test]
    fn test_parse_wire_command_axis() {
        let cmd = parse_wire_command("CMD:0,2001,0.500000").unwrap();
        assert_eq!(cmd.device_id, 0);
        assert_eq!(cmd.command_id, 2001);
        assert!((cmd.value - 0.5).abs() < 1e-6);
        assert_eq!(cmd.action_type, DcsActionType::Axis);
    }

    #[test]
    fn test_parse_wire_command_button_press() {
        let cmd = parse_wire_command("BTN:25,3001,1.000000").unwrap();
        assert_eq!(cmd.device_id, 25);
        assert_eq!(cmd.command_id, 3001);
        assert_eq!(cmd.action_type, DcsActionType::ButtonPress);
    }

    #[test]
    fn test_parse_wire_command_button_release() {
        let cmd = parse_wire_command("BTN:25,3001,0.000000").unwrap();
        assert_eq!(cmd.action_type, DcsActionType::ButtonRelease);
    }

    #[test]
    fn test_parse_wire_command_toggle() {
        let cmd = parse_wire_command("TGL:2,500,1.000000").unwrap();
        assert_eq!(cmd.action_type, DcsActionType::Toggle);
    }

    #[test]
    fn test_parse_wire_command_unknown_prefix() {
        let res = parse_wire_command("XYZ:0,1,0.5");
        assert!(matches!(res, Err(WireParseError::UnknownPrefix(_))));
    }

    #[test]
    fn test_parse_wire_command_bad_field_count() {
        let res = parse_wire_command("CMD:0,1");
        assert!(matches!(
            res,
            Err(WireParseError::BadFieldCount {
                expected: 3,
                got: 2
            })
        ));
    }

    #[test]
    fn test_parse_wire_command_invalid_number() {
        let res = parse_wire_command("CMD:abc,1,0.5");
        assert!(matches!(res, Err(WireParseError::InvalidNumber(_))));
    }

    #[test]
    fn test_parse_wire_roundtrip() {
        let original = DcsControlCommand::axis(0, 2001, 0.75);
        let wire = original.to_wire();
        let parsed = parse_wire_command(&wire).unwrap();
        assert_eq!(parsed.device_id, original.device_id);
        assert_eq!(parsed.command_id, original.command_id);
        assert!((parsed.value - original.value).abs() < 1e-6);
    }

    #[test]
    fn test_parse_wire_payload() {
        let payload = "CMD:0,2001,0.5\nBTN:25,3001,1.0\nTGL:2,500,1.0\n";
        let results = parse_wire_payload(payload);
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    // --- Clickable tests ---

    #[test]
    fn test_clickable_command() {
        let master_arm = Clickable {
            label: "Master Arm",
            device_id: 1,
            button: 100,
            min_value: 0.0,
            max_value: 1.0,
        };
        let cmd = master_arm.command(0.5);
        assert_eq!(cmd.device_id, 1);
        assert_eq!(cmd.command_id, 100);
        assert!((cmd.value - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_clickable_press_release() {
        let sw = Clickable {
            label: "Test Switch",
            device_id: 5,
            button: 200,
            min_value: 0.0,
            max_value: 1.0,
        };
        let press = sw.press();
        assert!((press.value - 1.0).abs() < f64::EPSILON);
        let release = sw.release();
        assert!(release.value.abs() < f64::EPSILON);
    }

    #[test]
    fn test_clickable_clamps_value() {
        let knob = Clickable {
            label: "Volume",
            device_id: 5,
            button: 300,
            min_value: 0.0,
            max_value: 1.0,
        };
        let cmd = knob.command(2.0);
        assert!((cmd.value - 1.0).abs() < f64::EPSILON);
    }

    // --- Per-aircraft axis mapping tests ---

    #[test]
    fn test_lookup_aircraft_axis_fa18c() {
        let mapping = lookup_aircraft_axis("FA-18C", "pitch").unwrap();
        assert_eq!(mapping.device_id, 0);
        assert_eq!(mapping.command_id, 2001);
    }

    #[test]
    fn test_lookup_aircraft_axis_f16c() {
        let mapping = lookup_aircraft_axis("F-16C", "throttle").unwrap();
        assert_eq!(mapping.device_id, 0);
        assert_eq!(mapping.command_id, 2004);
    }

    #[test]
    fn test_lookup_aircraft_axis_unknown_module() {
        assert!(lookup_aircraft_axis("MiG-29", "pitch").is_none());
    }

    #[test]
    fn test_lookup_aircraft_axis_unknown_axis() {
        assert!(lookup_aircraft_axis("FA-18C", "flaps").is_none());
    }

    #[test]
    fn test_wire_parse_error_display() {
        assert_eq!(
            WireParseError::UnknownPrefix("XYZ".into()).to_string(),
            "unknown prefix: XYZ"
        );
        assert_eq!(
            WireParseError::BadFieldCount {
                expected: 3,
                got: 2
            }
            .to_string(),
            "expected 3 fields, got 2"
        );
    }
}
