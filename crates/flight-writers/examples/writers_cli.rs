//! Example CLI for the Writers system

use clap::{Parser, Subcommand};
use flight_writers::*;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "writers-cli")]
#[command(about = "Flight Hub Writers CLI - Manage simulator configurations")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, default_value = "./config")]
    config_dir: PathBuf,

    #[arg(long, default_value = "./golden")]
    golden_dir: PathBuf,

    #[arg(long, default_value = "./backups")]
    backup_dir: PathBuf,
}

#[derive(Subcommand)]
enum Commands {
    /// Apply a writer configuration
    Apply {
        /// Path to the writer configuration file
        config_file: PathBuf,
    },
    /// Verify simulator configuration
    Verify {
        /// Simulator type (msfs, xplane, dcs)
        sim: String,
        /// Simulator version
        version: String,
    },
    /// Repair simulator configuration
    Repair {
        /// Simulator type (msfs, xplane, dcs)
        sim: String,
        /// Simulator version
        version: String,
    },
    /// Rollback to a previous configuration
    Rollback {
        /// Backup ID to rollback to
        backup_id: String,
    },
    /// Run golden file tests
    Test {
        /// Simulator type (msfs, xplane, dcs)
        sim: String,
    },
    /// List available backups
    ListBackups,
    /// Generate coverage matrix
    Coverage {
        /// Simulator type (msfs, xplane, dcs)
        sim: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let writers = Writers::new(&cli.config_dir, &cli.golden_dir, &cli.backup_dir)?;

    match cli.command {
        Commands::Apply { config_file } => {
            println!("Applying configuration from {:?}", config_file);

            let config_content = std::fs::read_to_string(&config_file)?;
            let config: WriterConfig = serde_json::from_str(&config_content)?;

            let result = writers.apply_writer(&config).await?;

            if result.success {
                println!("✅ Successfully applied configuration");
                println!("   Modified files: {}", result.modified_files.len());
                println!("   Backup ID: {}", result.backup_id);

                for file in &result.modified_files {
                    println!("   📝 {}", file.display());
                }
            } else {
                println!("❌ Failed to apply configuration");
                for error in &result.errors {
                    println!("   Error: {}", error);
                }
            }
        }

        Commands::Verify { sim, version } => {
            println!("Verifying {} configuration for version {}", sim, version);

            let sim_type = parse_simulator_type(&sim)?;
            let result = writers.verify(sim_type, &version).await?;

            if result.success {
                println!("✅ Configuration verification passed");
            } else {
                println!("❌ Configuration verification failed");

                if !result.script_results.is_empty() {
                    println!("\n📋 Script Results:");
                    for script in &result.script_results {
                        let status = if script.success { "✅" } else { "❌" };
                        println!(
                            "   {} {}: {}",
                            status,
                            script.name,
                            if script.success { "PASS" } else { "FAIL" }
                        );

                        if !script.success {
                            for error in &script.errors {
                                println!("      Error: {}", error);
                            }
                        }
                    }
                }

                if !result.mismatched_files.is_empty() {
                    println!("\n📁 Mismatched Files:");
                    for mismatch in &result.mismatched_files {
                        println!(
                            "   ❌ {}: {:?}",
                            mismatch.file.display(),
                            mismatch.mismatch_type
                        );
                    }
                }
            }
        }

        Commands::Repair { sim, version } => {
            println!("Repairing {} configuration for version {}", sim, version);

            let sim_type = parse_simulator_type(&sim)?;
            let verify_result = writers.verify(sim_type, &version).await?;

            if verify_result.success {
                println!("✅ No repair needed - configuration is already correct");
                return Ok(());
            }

            let repair_result = writers.repair(&verify_result).await?;

            if repair_result.success {
                println!("✅ Successfully repaired configuration");
                println!("   Repaired files: {}", repair_result.repaired_files.len());
                println!("   Backup ID: {}", repair_result.backup_id);

                for file in &repair_result.repaired_files {
                    println!("   🔧 {}", file.display());
                }
            } else {
                println!("❌ Failed to repair configuration");
                for error in &repair_result.errors {
                    println!("   Error: {}", error);
                }
            }
        }

        Commands::Rollback { backup_id } => {
            println!("Rolling back to backup {}", backup_id);

            let result = writers.rollback(&backup_id).await?;

            if result.success {
                println!("✅ Successfully rolled back configuration");
                println!("   Restored files: {}", result.restored_files.len());

                for file in &result.restored_files {
                    println!("   ↩️  {}", file.display());
                }
            } else {
                println!("❌ Failed to rollback configuration");
                for error in &result.errors {
                    println!("   Error: {}", error);
                }
            }
        }

        Commands::Test { sim } => {
            println!("Running golden file tests for {}", sim);

            let sim_type = parse_simulator_type(&sim)?;
            let result = writers.test_golden_files(sim_type).await?;

            println!("\n📊 Test Results:");
            println!(
                "   Overall: {}",
                if result.success {
                    "✅ PASS"
                } else {
                    "❌ FAIL"
                }
            );
            println!(
                "   Test cases: {}/{} passed",
                result.test_cases.iter().filter(|tc| tc.success).count(),
                result.test_cases.len()
            );

            for test_case in &result.test_cases {
                let status = if test_case.success { "✅" } else { "❌" };
                println!("   {} {}", status, test_case.name);

                if !test_case.success {
                    if let Some(diff) = &test_case.diff {
                        println!("      Diff:");
                        for line in diff.lines().take(10) {
                            println!("        {}", line);
                        }
                        if diff.lines().count() > 10 {
                            println!("        ... (truncated)");
                        }
                    }
                }
            }

            println!("\n📈 Coverage Matrix:");
            println!("   Coverage: {:.1}%", result.coverage.coverage_percent);
            println!(
                "   Versions: {} ({})",
                result.coverage.versions.len(),
                result.coverage.versions.join(", ")
            );
            println!(
                "   Areas: {} ({})",
                result.coverage.areas.len(),
                result.coverage.areas.join(", ")
            );

            if !result.coverage.missing_coverage.is_empty() {
                println!("   Missing coverage:");
                for missing in &result.coverage.missing_coverage {
                    println!("     - {}", missing);
                }
            }
        }

        Commands::ListBackups => {
            println!("Available backups:");

            let rollback_manager = RollbackManager::new(&cli.backup_dir);
            let backups = rollback_manager.list_backups().await?;

            if backups.is_empty() {
                println!("   No backups found");
            } else {
                for backup in &backups {
                    let timestamp = chrono::DateTime::from_timestamp(backup.timestamp as i64, 0)
                        .unwrap_or_default();
                    println!(
                        "   📦 {} ({})",
                        backup.id,
                        timestamp.format("%Y-%m-%d %H:%M:%S")
                    );
                    println!("      Sim: {} v{}", backup.sim, backup.version);
                    println!("      Files: {}", backup.files.len());
                    println!("      Description: {}", backup.description);
                    println!();
                }
            }
        }

        Commands::Coverage { sim } => {
            println!("Generating coverage matrix for {}", sim);

            let sim_type = parse_simulator_type(&sim)?;
            let result = writers.test_golden_files(sim_type).await?;

            println!("\n📊 Coverage Matrix for {}:", sim);
            println!("┌─────────────────────┬─────────────────────┐");
            println!("│ Metric              │ Value               │");
            println!("├─────────────────────┼─────────────────────┤");
            println!(
                "│ Overall Coverage    │ {:.1}%              │",
                result.coverage.coverage_percent
            );
            println!(
                "│ Versions Covered    │ {}                  │",
                result.coverage.versions.len()
            );
            println!(
                "│ Areas Covered       │ {}                  │",
                result.coverage.areas.len()
            );
            println!(
                "│ Test Cases          │ {}                  │",
                result.test_cases.len()
            );
            println!(
                "│ Passing Tests       │ {}                  │",
                result.test_cases.iter().filter(|tc| tc.success).count()
            );
            println!("└─────────────────────┴─────────────────────┘");

            if !result.coverage.versions.is_empty() {
                println!("\n📋 Covered Versions:");
                for version in &result.coverage.versions {
                    println!("   • {}", version);
                }
            }

            if !result.coverage.areas.is_empty() {
                println!("\n🎯 Covered Areas:");
                for area in &result.coverage.areas {
                    println!("   • {}", area);
                }
            }

            if !result.coverage.missing_coverage.is_empty() {
                println!("\n⚠️  Missing Coverage:");
                for missing in &result.coverage.missing_coverage {
                    println!("   • {}", missing);
                }
            }
        }
    }

    Ok(())
}

fn parse_simulator_type(sim: &str) -> Result<SimulatorType, Box<dyn std::error::Error>> {
    match sim.to_lowercase().as_str() {
        "msfs" => Ok(SimulatorType::MSFS),
        "xplane" => Ok(SimulatorType::XPlane),
        "dcs" => Ok(SimulatorType::DCS),
        _ => Err(format!(
            "Unknown simulator type: {}. Use 'msfs', 'xplane', or 'dcs'",
            sim
        )
        .into()),
    }
}
