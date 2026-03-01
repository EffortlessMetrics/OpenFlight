// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS-BIOS import protocol command builder.
//!
//! The import protocol uses plain-text commands sent to UDP port 7778:
//! `CONTROL_NAME VALUE\n`
//!
//! This module provides a builder for constructing these commands.

use std::fmt;

/// Returns `true` if the string contains `\n` or `\r`, which would corrupt
/// the DCS-BIOS text protocol wire format.
fn contains_line_break(s: &str) -> bool {
    s.contains('\n') || s.contains('\r')
}

/// A DCS-BIOS import command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DcsBiosCommand {
    /// The control identifier.
    pub control: String,
    /// The argument value.
    pub argument: String,
}

impl DcsBiosCommand {
    /// Create a "set state" command to set a control to a specific integer value.
    ///
    /// # Example
    /// ```
    /// use flight_dcs_bios::DcsBiosCommand;
    /// let cmd = DcsBiosCommand::set_state("MASTER_ARM_SW", 1);
    /// assert_eq!(cmd.to_string(), "MASTER_ARM_SW 1\n");
    /// ```
    #[must_use]
    pub fn set_state(control: &str, value: u16) -> Self {
        assert!(
            !contains_line_break(control),
            "control name must not contain newlines"
        );
        Self {
            control: control.to_owned(),
            argument: value.to_string(),
        }
    }

    /// Create a "fixed step" command to increment/decrement a control.
    ///
    /// Positive delta → `INC`, negative → `DEC`. Returns `None` for zero delta.
    ///
    /// # Example
    /// ```
    /// use flight_dcs_bios::DcsBiosCommand;
    /// let cmd = DcsBiosCommand::fixed_step("HDG_SET", 1).unwrap();
    /// assert_eq!(cmd.to_string(), "HDG_SET INC\n");
    /// assert!(DcsBiosCommand::fixed_step("HDG_SET", 0).is_none());
    /// ```
    #[must_use]
    pub fn fixed_step(control: &str, delta: i32) -> Option<Self> {
        assert!(
            !contains_line_break(control),
            "control name must not contain newlines"
        );
        let argument = if delta > 0 {
            "INC".to_owned()
        } else if delta < 0 {
            "DEC".to_owned()
        } else {
            return None;
        };
        Some(Self {
            control: control.to_owned(),
            argument,
        })
    }

    /// Create an "action" command (press a button).
    ///
    /// # Example
    /// ```
    /// use flight_dcs_bios::DcsBiosCommand;
    /// let cmd = DcsBiosCommand::action("UFC_1");
    /// assert_eq!(cmd.to_string(), "UFC_1 1\n");
    /// ```
    #[must_use]
    pub fn action(control: &str) -> Self {
        assert!(
            !contains_line_break(control),
            "control name must not contain newlines"
        );
        Self {
            control: control.to_owned(),
            argument: "1".to_owned(),
        }
    }

    /// Create a command with a custom string argument.
    ///
    /// Returns `None` if `control` or `argument` contains `\n` or `\r`,
    /// which would corrupt the wire format.
    #[must_use]
    pub fn custom(control: &str, argument: &str) -> Option<Self> {
        if contains_line_break(control) || contains_line_break(argument) {
            return None;
        }
        Some(Self {
            control: control.to_owned(),
            argument: argument.to_owned(),
        })
    }

    /// Serialize this command to its wire format bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.to_string().into_bytes()
    }
}

impl fmt::Display for DcsBiosCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} {}", self.control, self.argument)
    }
}

/// A batch of DCS-BIOS commands to send together.
#[derive(Debug, Clone, Default)]
pub struct DcsBiosCommandBatch {
    commands: Vec<DcsBiosCommand>,
}

impl DcsBiosCommandBatch {
    /// Create a new empty batch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a command to the batch.
    pub fn push(&mut self, command: DcsBiosCommand) {
        self.commands.push(command);
    }

    /// Add a set-state command to the batch.
    pub fn set_state(&mut self, control: &str, value: u16) {
        self.push(DcsBiosCommand::set_state(control, value));
    }

    /// Add a fixed-step command to the batch. Does nothing for zero delta.
    pub fn fixed_step(&mut self, control: &str, delta: i32) {
        if let Some(cmd) = DcsBiosCommand::fixed_step(control, delta) {
            self.push(cmd);
        }
    }

    /// Add an action command to the batch.
    pub fn action(&mut self, control: &str) {
        self.push(DcsBiosCommand::action(control));
    }

    /// Number of commands in the batch.
    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether the batch is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Get all commands.
    #[must_use]
    pub fn commands(&self) -> &[DcsBiosCommand] {
        &self.commands
    }

    /// Serialize the entire batch to wire format bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for cmd in &self.commands {
            bytes.extend_from_slice(&cmd.to_bytes());
        }
        bytes
    }

    /// Clear all commands from the batch.
    pub fn clear(&mut self) {
        self.commands.clear();
    }
}

impl fmt::Display for DcsBiosCommandBatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for cmd in &self.commands {
            write!(f, "{cmd}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_state_command_format() {
        let cmd = DcsBiosCommand::set_state("MASTER_ARM_SW", 1);
        assert_eq!(cmd.to_string(), "MASTER_ARM_SW 1\n");
        assert_eq!(cmd.control, "MASTER_ARM_SW");
        assert_eq!(cmd.argument, "1");
    }

    #[test]
    fn set_state_zero() {
        let cmd = DcsBiosCommand::set_state("MASTER_ARM_SW", 0);
        assert_eq!(cmd.to_string(), "MASTER_ARM_SW 0\n");
    }

    #[test]
    fn set_state_large_value() {
        let cmd = DcsBiosCommand::set_state("BARO_SET", 65535);
        assert_eq!(cmd.to_string(), "BARO_SET 65535\n");
    }

    #[test]
    fn fixed_step_increment() {
        let cmd = DcsBiosCommand::fixed_step("HDG_SET", 1).unwrap();
        assert_eq!(cmd.to_string(), "HDG_SET INC\n");
    }

    #[test]
    fn fixed_step_decrement() {
        let cmd = DcsBiosCommand::fixed_step("HDG_SET", -1).unwrap();
        assert_eq!(cmd.to_string(), "HDG_SET DEC\n");
    }

    #[test]
    fn fixed_step_zero_returns_none() {
        assert!(DcsBiosCommand::fixed_step("HDG_SET", 0).is_none());
    }

    #[test]
    fn action_command_format() {
        let cmd = DcsBiosCommand::action("UFC_1");
        assert_eq!(cmd.to_string(), "UFC_1 1\n");
    }

    #[test]
    fn custom_command() {
        let cmd = DcsBiosCommand::custom("RADIO_FREQ", "251000").unwrap();
        assert_eq!(cmd.to_string(), "RADIO_FREQ 251000\n");
    }

    #[test]
    fn custom_rejects_newlines_in_control() {
        assert!(DcsBiosCommand::custom("BAD\nCTRL", "value").is_none());
        assert!(DcsBiosCommand::custom("BAD\rCTRL", "value").is_none());
    }

    #[test]
    fn custom_rejects_newlines_in_argument() {
        assert!(DcsBiosCommand::custom("CTRL", "bad\nvalue").is_none());
        assert!(DcsBiosCommand::custom("CTRL", "bad\rvalue").is_none());
    }

    #[test]
    fn to_bytes_matches_string() {
        let cmd = DcsBiosCommand::set_state("TEST", 42);
        assert_eq!(cmd.to_bytes(), b"TEST 42\n");
    }

    #[test]
    fn batch_collects_commands() {
        let mut batch = DcsBiosCommandBatch::new();
        assert!(batch.is_empty());

        batch.set_state("MASTER_ARM_SW", 1);
        batch.action("UFC_1");
        batch.fixed_step("HDG_SET", 1);

        assert_eq!(batch.len(), 3);
        assert!(!batch.is_empty());
    }

    #[test]
    fn batch_to_bytes() {
        let mut batch = DcsBiosCommandBatch::new();
        batch.set_state("SW1", 1);
        batch.set_state("SW2", 0);

        let bytes = batch.to_bytes();
        let expected = b"SW1 1\nSW2 0\n";
        assert_eq!(bytes, expected);
    }

    #[test]
    fn batch_display() {
        let mut batch = DcsBiosCommandBatch::new();
        batch.action("BTN1");
        batch.action("BTN2");

        let s = batch.to_string();
        assert_eq!(s, "BTN1 1\nBTN2 1\n");
    }

    #[test]
    fn batch_clear() {
        let mut batch = DcsBiosCommandBatch::new();
        batch.action("BTN1");
        assert_eq!(batch.len(), 1);
        batch.clear();
        assert!(batch.is_empty());
    }

    #[test]
    fn command_equality() {
        let a = DcsBiosCommand::set_state("SW", 1);
        let b = DcsBiosCommand::set_state("SW", 1);
        let c = DcsBiosCommand::set_state("SW", 0);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
