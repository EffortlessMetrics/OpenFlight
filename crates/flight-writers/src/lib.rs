// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Flight Writers - Versioned configuration management for flight simulators
//!
//! This crate provides a table-driven approach to managing simulator configurations
//! through versioned JSON diffs, golden file testing, and verify/repair functionality.

pub mod diff;
pub mod golden;
pub mod repair;
pub mod rollback;
pub mod types;
pub mod verify;
pub mod curve_conflict;

pub use diff::*;
pub use golden::*;
pub use repair::*;
pub use rollback::*;
pub use types::*;
pub use verify::*;
pub use curve_conflict::*;

use anyhow::Result;
use std::path::Path;

/// Main Writers API for managing simulator configurations
pub struct Writers {
    config_dir: std::path::PathBuf,
    golden_dir: std::path::PathBuf,
    backup_dir: std::path::PathBuf,
}

impl Writers {
    /// Create a new Writers instance with the specified directories
    pub fn new<P: AsRef<Path>>(config_dir: P, golden_dir: P, backup_dir: P) -> Result<Self> {
        let config_dir = config_dir.as_ref().to_path_buf();
        let golden_dir = golden_dir.as_ref().to_path_buf();
        let backup_dir = backup_dir.as_ref().to_path_buf();

        // Ensure directories exist
        std::fs::create_dir_all(&config_dir)?;
        std::fs::create_dir_all(&golden_dir)?;
        std::fs::create_dir_all(&backup_dir)?;

        Ok(Self {
            config_dir,
            golden_dir,
            backup_dir,
        })
    }

    /// Apply a writer configuration to the target simulator
    pub async fn apply_writer(&self, writer: &WriterConfig) -> Result<ApplyResult> {
        let applier = WriterApplier::new(&self.backup_dir);
        applier.apply(writer).await
    }

    /// Verify current simulator configuration against expected state
    pub async fn verify(&self, sim: SimulatorType, version: &str) -> Result<VerifyResult> {
        let verifier = ConfigVerifier::new(&self.golden_dir);
        verifier.verify(sim, version).await
    }

    /// Repair simulator configuration by applying minimal diffs
    pub async fn repair(&self, verify_result: &VerifyResult) -> Result<RepairResult> {
        let repairer = ConfigRepairer::new(&self.config_dir, &self.backup_dir);
        repairer.repair(verify_result).await
    }

    /// Rollback to previous configuration
    pub async fn rollback(&self, rollback_id: &str) -> Result<RollbackResult> {
        let rollback_manager = RollbackManager::new(&self.backup_dir);
        rollback_manager.rollback(rollback_id).await
    }

    /// Run golden file tests for the specified simulator
    pub async fn test_golden_files(&self, sim: SimulatorType) -> Result<GoldenTestResult> {
        let tester = GoldenFileTester::new(&self.golden_dir);
        tester.test_simulator(sim).await
    }
}
