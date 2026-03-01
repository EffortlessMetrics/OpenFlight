// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Write batching and conflict resolution for sim variable writes.
//!
//! Coordinates multiple axis/variable writes into batches, resolves conflicts
//! when multiple sources write to the same variable using priority ordering,
//! and produces a final list of [`WriteCommand`]s per tick.

use crate::types::SimulatorType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single write command to be sent to a simulator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WriteCommand {
    /// Target sim variable name (sim-specific)
    pub sim_var: String,
    /// Value to write
    pub value: f64,
    /// Unit for the value
    pub unit: String,
    /// Source identifier (e.g. "profile:cessna-172", "panel:mcp")
    pub source: String,
    /// Priority — higher values override lower for the same variable.
    pub priority: u32,
}

/// Collects multiple axis/variable writes into one batch.
#[derive(Debug, Clone, Default)]
pub struct WriteBatch {
    commands: Vec<WriteCommand>,
}

impl WriteBatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a write command to the batch.
    pub fn push(&mut self, cmd: WriteCommand) {
        self.commands.push(cmd);
    }

    /// Number of commands in this batch.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Iterate over commands.
    pub fn iter(&self) -> impl Iterator<Item = &WriteCommand> {
        self.commands.iter()
    }

    /// Consume the batch into a `Vec<WriteCommand>`.
    pub fn into_commands(self) -> Vec<WriteCommand> {
        self.commands
    }
}

/// Strategy for resolving conflicts when multiple sources write to the same
/// sim variable within a single tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictStrategy {
    /// Highest priority wins.  Among equal priorities, last-write wins.
    Priority,
    /// Last write in submission order always wins regardless of priority.
    LastWriteWins,
}

impl Default for ConflictStrategy {
    fn default() -> Self {
        Self::Priority
    }
}

/// Resolves write conflicts according to a [`ConflictStrategy`].
#[derive(Debug, Clone)]
pub struct ConflictResolver {
    strategy: ConflictStrategy,
}

impl Default for ConflictResolver {
    fn default() -> Self {
        Self::new(ConflictStrategy::Priority)
    }
}

impl ConflictResolver {
    pub fn new(strategy: ConflictStrategy) -> Self {
        Self { strategy }
    }

    /// Resolve a list of potentially conflicting commands into a
    /// deduplicated set.  For each `sim_var` the winning command is chosen
    /// according to the active strategy.
    pub fn resolve(&self, commands: Vec<WriteCommand>) -> Vec<WriteCommand> {
        let mut winners: HashMap<String, WriteCommand> = HashMap::new();

        for cmd in commands {
            let key = cmd.sim_var.clone();

            match self.strategy {
                ConflictStrategy::Priority => {
                    let replace = match winners.get(&key) {
                        Some(existing) => cmd.priority >= existing.priority,
                        None => true,
                    };
                    if replace {
                        winners.insert(key, cmd);
                    }
                }
                ConflictStrategy::LastWriteWins => {
                    winners.insert(key, cmd);
                }
            }
        }

        let mut result: Vec<WriteCommand> = winners.into_values().collect();
        result.sort_by(|a, b| a.sim_var.cmp(&b.sim_var));
        result
    }
}

/// Coordinates writes across ticks.
///
/// Accumulate one or more [`WriteBatch`] instances, then call
/// [`commit`](WriterEngine::commit) to produce the final list of write
/// commands after conflict resolution.
#[derive(Debug)]
pub struct WriterEngine {
    resolver: ConflictResolver,
    sim: SimulatorType,
    pending: Vec<WriteBatch>,
}

impl WriterEngine {
    pub fn new(sim: SimulatorType, strategy: ConflictStrategy) -> Self {
        Self {
            resolver: ConflictResolver::new(strategy),
            sim,
            pending: Vec::new(),
        }
    }

    /// Target simulator for this engine.
    pub fn sim(&self) -> SimulatorType {
        self.sim
    }

    /// Submit a batch for the current tick.
    pub fn submit(&mut self, batch: WriteBatch) {
        self.pending.push(batch);
    }

    /// Commit all pending batches, returning the resolved write commands.
    ///
    /// The pending queue is drained after this call.
    pub fn commit(&mut self) -> Vec<WriteCommand> {
        let all_commands: Vec<WriteCommand> = self
            .pending
            .drain(..)
            .flat_map(|b| b.into_commands())
            .collect();
        self.resolver.resolve(all_commands)
    }

    /// Number of pending batches.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Discard all pending batches without committing.
    pub fn discard(&mut self) {
        self.pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(var: &str, value: f64, source: &str, priority: u32) -> WriteCommand {
        WriteCommand {
            sim_var: var.to_string(),
            value,
            unit: "position".to_string(),
            source: source.to_string(),
            priority,
        }
    }

    // ── WriteBatch ───────────────────────────────────────────────

    #[test]
    fn batch_new_is_empty() {
        let batch = WriteBatch::new();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);
    }

    #[test]
    fn batch_push_and_len() {
        let mut batch = WriteBatch::new();
        batch.push(cmd("AILERON", 0.5, "profile", 10));
        batch.push(cmd("ELEVATOR", -0.3, "profile", 10));
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn batch_into_commands() {
        let mut batch = WriteBatch::new();
        batch.push(cmd("AILERON", 0.5, "profile", 10));
        let cmds = batch.into_commands();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].sim_var, "AILERON");
    }

    // ── ConflictResolver: Priority ───────────────────────────────

    #[test]
    fn priority_higher_wins() {
        let resolver = ConflictResolver::new(ConflictStrategy::Priority);
        let commands = vec![
            cmd("AILERON", 0.5, "profile:global", 10),
            cmd("AILERON", 0.8, "profile:aircraft", 20),
        ];
        let result = resolver.resolve(commands);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, 0.8);
        assert_eq!(result[0].source, "profile:aircraft");
    }

    #[test]
    fn priority_equal_last_write_wins() {
        let resolver = ConflictResolver::new(ConflictStrategy::Priority);
        let commands = vec![
            cmd("AILERON", 0.5, "profile:a", 10),
            cmd("AILERON", 0.8, "profile:b", 10),
        ];
        let result = resolver.resolve(commands);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, 0.8);
    }

    #[test]
    fn priority_lower_loses() {
        let resolver = ConflictResolver::new(ConflictStrategy::Priority);
        let commands = vec![
            cmd("AILERON", 0.8, "profile:aircraft", 20),
            cmd("AILERON", 0.5, "profile:global", 10),
        ];
        let result = resolver.resolve(commands);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, 0.8);
    }

    #[test]
    fn priority_different_vars_no_conflict() {
        let resolver = ConflictResolver::new(ConflictStrategy::Priority);
        let commands = vec![
            cmd("AILERON", 0.5, "profile", 10),
            cmd("ELEVATOR", -0.3, "profile", 10),
        ];
        let result = resolver.resolve(commands);
        assert_eq!(result.len(), 2);
    }

    // ── ConflictResolver: LastWriteWins ──────────────────────────

    #[test]
    fn last_write_wins_ignores_priority() {
        let resolver = ConflictResolver::new(ConflictStrategy::LastWriteWins);
        let commands = vec![
            cmd("AILERON", 0.5, "high-priority", 100),
            cmd("AILERON", 0.1, "low-priority", 1),
        ];
        let result = resolver.resolve(commands);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, 0.1);
    }

    // ── WriterEngine ─────────────────────────────────────────────

    #[test]
    fn engine_submit_and_commit() {
        let mut engine = WriterEngine::new(SimulatorType::MSFS, ConflictStrategy::Priority);

        let mut batch = WriteBatch::new();
        batch.push(cmd("AILERON", 0.5, "profile", 10));
        batch.push(cmd("ELEVATOR", -0.3, "profile", 10));
        engine.submit(batch);

        assert_eq!(engine.pending_count(), 1);

        let result = engine.commit();
        assert_eq!(result.len(), 2);
        assert_eq!(engine.pending_count(), 0);
    }

    #[test]
    fn engine_multiple_batches_with_conflict() {
        let mut engine = WriterEngine::new(SimulatorType::MSFS, ConflictStrategy::Priority);

        let mut batch1 = WriteBatch::new();
        batch1.push(cmd("AILERON", 0.5, "profile:global", 10));
        engine.submit(batch1);

        let mut batch2 = WriteBatch::new();
        batch2.push(cmd("AILERON", 0.8, "profile:aircraft", 20));
        batch2.push(cmd("RUDDER", 0.1, "profile:aircraft", 20));
        engine.submit(batch2);

        let result = engine.commit();
        assert_eq!(result.len(), 2);

        let aileron = result.iter().find(|c| c.sim_var == "AILERON").unwrap();
        assert_eq!(aileron.value, 0.8);
        assert_eq!(aileron.source, "profile:aircraft");
    }

    #[test]
    fn engine_discard_clears_pending() {
        let mut engine = WriterEngine::new(SimulatorType::XPlane, ConflictStrategy::Priority);

        let mut batch = WriteBatch::new();
        batch.push(cmd("AILERON", 0.5, "profile", 10));
        engine.submit(batch);
        assert_eq!(engine.pending_count(), 1);

        engine.discard();
        assert_eq!(engine.pending_count(), 0);

        let result = engine.commit();
        assert!(result.is_empty());
    }

    #[test]
    fn engine_sim_accessor() {
        let engine = WriterEngine::new(SimulatorType::DCS, ConflictStrategy::Priority);
        assert_eq!(engine.sim(), SimulatorType::DCS);
    }

    #[test]
    fn engine_commit_empty_is_empty() {
        let mut engine = WriterEngine::new(SimulatorType::MSFS, ConflictStrategy::Priority);
        assert!(engine.commit().is_empty());
    }

    // ── Profile cascade simulation ───────────────────────────────

    #[test]
    fn four_level_profile_cascade() {
        let mut engine = WriterEngine::new(SimulatorType::MSFS, ConflictStrategy::Priority);

        // Global profile (lowest priority)
        let mut global = WriteBatch::new();
        global.push(cmd("AILERON", 0.0, "profile:global", 10));
        global.push(cmd("ELEVATOR", 0.0, "profile:global", 10));
        global.push(cmd("RUDDER", 0.0, "profile:global", 10));
        engine.submit(global);

        // Simulator profile
        let mut sim = WriteBatch::new();
        sim.push(cmd("AILERON", 0.1, "profile:msfs", 20));
        engine.submit(sim);

        // Aircraft profile
        let mut aircraft = WriteBatch::new();
        aircraft.push(cmd("AILERON", 0.3, "profile:c172", 30));
        aircraft.push(cmd("RUDDER", 0.2, "profile:c172", 30));
        engine.submit(aircraft);

        // Phase-of-flight profile (highest priority)
        let mut phase = WriteBatch::new();
        phase.push(cmd("AILERON", 0.5, "profile:landing", 40));
        engine.submit(phase);

        let result = engine.commit();
        assert_eq!(result.len(), 3); // AILERON, ELEVATOR, RUDDER

        let aileron = result.iter().find(|c| c.sim_var == "AILERON").unwrap();
        assert_eq!(aileron.value, 0.5);
        assert_eq!(aileron.source, "profile:landing");

        let elevator = result.iter().find(|c| c.sim_var == "ELEVATOR").unwrap();
        assert_eq!(elevator.value, 0.0);
        assert_eq!(elevator.source, "profile:global");

        let rudder = result.iter().find(|c| c.sim_var == "RUDDER").unwrap();
        assert_eq!(rudder.value, 0.2);
        assert_eq!(rudder.source, "profile:c172");
    }

    // ── WriteCommand serialization ───────────────────────────────

    #[test]
    fn write_command_round_trip() {
        let command = cmd("AILERON POSITION", 0.75, "profile:test", 15);
        let json = serde_json::to_string(&command).unwrap();
        let deserialized: WriteCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, command);
    }

    // ── Default implementations ──────────────────────────────────

    #[test]
    fn conflict_strategy_default_is_priority() {
        assert_eq!(ConflictStrategy::default(), ConflictStrategy::Priority);
    }

    #[test]
    fn resolver_default_uses_priority() {
        let resolver = ConflictResolver::default();
        let commands = vec![cmd("A", 1.0, "low", 1), cmd("A", 2.0, "high", 100)];
        let result = resolver.resolve(commands);
        assert_eq!(result[0].value, 2.0);
    }
}
