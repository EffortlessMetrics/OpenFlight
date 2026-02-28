// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS control command injection
//!
//! Serialises axis and button commands into the DCS Export.lua protocol
//! format for sending back to DCS via UDP. Commands are buffered and
//! flushed once per frame to avoid flooding the socket.

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
    /// ```
    ///
    /// Values are formatted with up to 6 decimal places.
    pub fn flush(&mut self) -> Vec<u8> {
        if self.buffer.is_empty() {
            return Vec::new();
        }

        let mut out = String::with_capacity(self.buffer.len() * 32);
        for cmd in self.buffer.drain(..) {
            out.push_str(&format!(
                "CMD:{},{},{:.6}\n",
                cmd.device_id, cmd.command_id, cmd.value
            ));
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn axis_cmd(device: u32, cmd: u32, val: f64) -> DcsControlCommand {
        DcsControlCommand {
            device_id: device,
            command_id: cmd,
            value: val,
        }
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
        inj.queue_command(axis_cmd(1, 1, std::f64::consts::PI));
        let text = String::from_utf8(inj.flush()).unwrap();
        // Should have 6 decimal places
        assert!(text.contains("3.141593"));
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
}
