#!/usr/bin/env cargo +nightly -Zscript
//! CI Dashboard Automation
//!
//! This script provides automation for supply chain health dashboard reporting.
//! It integrates with the CI gate controller and provides scheduled reporting.
//!
//! Requirements addressed: Task 9.3 - Weekly and quarterly reporting automation

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
struct AutomationConfig {
    enable_weekly_reports: bool,
    enable_quarterly_reports: bool,
    slack_webhook_url: Option<String>,
    email_recipients: Vec<String>,
    artifact_retention_days: u32,
}

#[derive(Debug)]
struct ReportScheduler {
    config: AutomationConfig,
}

impl AutomationConfig {
    fn from_env() -> Self {
        Self {
            enable_weekly_reports: std::env::var("ENABLE_WEEKLY_REPORTS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_quarterly_reports: std::env::var("ENABLE_QUARTERLY_REPORTS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            slack_webhook_url: std::env::var("SLACK_WEBHOOK_URL").ok(),
            email_recipients: std::env::var("EMAIL_RECIPIENTS")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.trim().to_string())
                .collect(),
            artifact_retention_days: std::env::var("ARTIFACT_RETENTION_DAYS")
                .unwrap_or_else(|_| "90".to_string())
                .parse()
                .unwrap_or(90),
        }
    }
}

impl ReportScheduler {
    fn new() -> Self {
        Self {
            config: AutomationConfig::from_env(),
        }
    }
    
    fn should_generate_weekly_report(&self) -> bool {
        if !self.config.enable_weekly_reports {
            return false;
        }
        
        // Check if it's Monday (day 1 of the week)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Simple day-of-week calculation (Monday = 1)
        let days_since_epoch = now / 86400;
        let day_of_week = (days_since_epoch + 4) % 7; // Adjust for epoch starting on Thursday
        
        day_of_week == 1 // Monday
    }
    
    fn should_generate_quarterly_report(&self) -> bool {
        if !self.config.enable_quarterly_reports {
            return false;
        }
        
        // Check if it's the first day of a quarter
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Simple date calculation - check if it's January 1, April 1, July 1, or October 1
        let days_since_epoch = now / 86400;
        let _year = 1970 + (days_since_epoch / 365);
        let day_of_year = days_since_epoch % 365;
        
        // Approximate quarter start days (simplified)
        matches!(day_of_year, 0 | 90 | 181 | 273) // Jan 1, Apr 1, Jul 1, Oct 1 (approximate)
    }
    
    fn run_dashboard_report(&self, report_type: &str) -> Result<String, String> {
        println!("🏥 Generating {} supply chain health report...", report_type);
        
        let output = Command::new("cargo")
            .args(&["+nightly", "-Zscript", "scripts/supply_chain_dashboard.rs", report_type])
            .output()
            .map_err(|e| format!("Failed to run dashboard script: {}", e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Dashboard script failed: {}", stderr));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Extract report file path from output
        for line in stdout.lines() {
            if line.contains("Detailed report saved to:") {
                if let Some(path_start) = line.find("target/ci-artifacts/") {
                    let path = line[path_start..].trim();
                    return Ok(path.to_string());
                }
            }
        }
        
        Err("Could not find generated report path".to_string())
    }
    
    fn send_notifications(&self, report_path: &str, report_type: &str) -> Result<(), String> {
        // Read report content for notifications
        let report_content = fs::read_to_string(report_path)
            .map_err(|e| format!("Failed to read report: {}", e))?;
        
        // Extract executive summary for notifications
        let summary = self.extract_summary(&report_content);
        
        // Send Slack notification if configured
        if let Some(webhook_url) = &self.config.slack_webhook_url {
            self.send_slack_notification(webhook_url, &summary, report_type, report_path)?;
        }
        
        // Send email notifications if configured
        if !self.config.email_recipients.is_empty() {
            self.send_email_notifications(&summary, report_type, report_path)?;
        }
        
        Ok(())
    }
    
    fn extract_summary(&self, report_content: &str) -> String {
        let mut summary = String::new();
        let mut in_summary = false;
        let mut in_metrics = false;
        
        for line in report_content.lines() {
            if line.starts_with("## Executive Summary") {
                in_summary = true;
                continue;
            }
            
            if line.starts_with("## Key Metrics") {
                in_summary = false;
                in_metrics = true;
                summary.push_str("\n**Key Metrics:**\n");
                continue;
            }
            
            if line.starts_with("## ") && in_metrics {
                break;
            }
            
            if in_summary || in_metrics {
                summary.push_str(line);
                summary.push('\n');
            }
        }
        
        summary
    }
    
    fn send_slack_notification(&self, webhook_url: &str, summary: &str, report_type: &str, report_path: &str) -> Result<(), String> {
        let payload = format!(
            r#"{{
                "text": "Supply Chain Health Dashboard - {} Report",
                "blocks": [
                    {{
                        "type": "header",
                        "text": {{
                            "type": "plain_text",
                            "text": "🏥 Supply Chain Health Dashboard - {} Report"
                        }}
                    }},
                    {{
                        "type": "section",
                        "text": {{
                            "type": "mrkdwn",
                            "text": "{}"
                        }}
                    }},
                    {{
                        "type": "section",
                        "text": {{
                            "type": "mrkdwn",
                            "text": "📄 *Full Report:* `{}`"
                        }}
                    }}
                ]
            }}"#,
            report_type,
            report_type,
            summary.replace('\n', "\\n").replace('"', "\\\""),
            report_path
        );
        
        // Use curl to send Slack notification
        let output = Command::new("curl")
            .args(&[
                "-X", "POST",
                "-H", "Content-type: application/json",
                "--data", &payload,
                webhook_url
            ])
            .output()
            .map_err(|e| format!("Failed to send Slack notification: {}", e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Slack notification failed: {}", stderr));
        }
        
        println!("✅ Slack notification sent successfully");
        Ok(())
    }
    
    fn send_email_notifications(&self, summary: &str, report_type: &str, report_path: &str) -> Result<(), String> {
        // Simple email notification using system mail command
        let subject = format!("Supply Chain Health Dashboard - {} Report", report_type);
        let body = format!(
            "Supply Chain Health Dashboard - {} Report\n\n{}\n\nFull report available at: {}\n\n---\nThis is an automated report from the DevSecOps pipeline.",
            report_type, summary, report_path
        );
        
        for recipient in &self.config.email_recipients {
            let output = Command::new("mail")
                .args(&["-s", &subject, recipient])
                .arg(&body)
                .output();
                
            match output {
                Ok(result) if result.status.success() => {
                    println!("✅ Email sent to {}", recipient);
                }
                Ok(_) => {
                    println!("⚠️ Failed to send email to {} (mail command failed)", recipient);
                }
                Err(_) => {
                    println!("⚠️ Failed to send email to {} (mail command not available)", recipient);
                }
            }
        }
        
        Ok(())
    }
    
    fn cleanup_old_artifacts(&self) -> Result<(), String> {
        let artifacts_dir = "target/ci-artifacts";
        
        if !Path::new(artifacts_dir).exists() {
            return Ok(());
        }
        
        let cutoff_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() - (self.config.artifact_retention_days as u64 * 86400);
        
        let mut cleaned_count = 0;
        
        if let Ok(entries) = fs::read_dir(artifacts_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(created) = metadata.created() {
                        let created_timestamp = created
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        
                        if created_timestamp < cutoff_time {
                            if let Err(e) = fs::remove_file(entry.path()) {
                                println!("⚠️ Failed to remove old artifact {:?}: {}", entry.path(), e);
                            } else {
                                cleaned_count += 1;
                            }
                        }
                    }
                }
            }
        }
        
        if cleaned_count > 0 {
            println!("🧹 Cleaned up {} old artifacts (older than {} days)", cleaned_count, self.config.artifact_retention_days);
        }
        
        Ok(())
    }
    
    fn run_integration_with_ci_gate(&self) -> Result<(), String> {
        println!("🔗 Integrating dashboard with CI gate controller...");
        
        // Run the main CI gate controller first
        let output = Command::new("cargo")
            .args(&["+nightly", "-Zscript", "scripts/ci_supply_chain_gate.rs"])
            .output()
            .map_err(|e| format!("Failed to run CI gate controller: {}", e))?;
        
        let gate_success = output.status.success();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        println!("CI Gate Result: {}", if gate_success { "✅ PASSED" } else { "❌ FAILED" });
        
        // Always generate an on-demand dashboard report after CI gates
        match self.run_dashboard_report("on-demand") {
            Ok(report_path) => {
                println!("✅ Dashboard report generated: {}", report_path);
                
                // Send notifications for failed gates or critical issues
                if !gate_success {
                    if let Err(e) = self.send_notifications(&report_path, "Critical Alert") {
                        println!("⚠️ Failed to send critical alert notifications: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("⚠️ Failed to generate dashboard report: {}", e);
            }
        }
        
        // Return the original gate result
        if gate_success {
            Ok(())
        } else {
            Err(format!("CI gates failed:\n{}\n{}", stdout, stderr))
        }
    }
}

fn main() {
    println!("🤖 CI Dashboard Automation");
    println!("==========================");
    
    let scheduler = ReportScheduler::new();
    
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("auto");
    
    match mode {
        "auto" => {
            // Automatic mode - check if reports should be generated
            println!("🔄 Running in automatic mode...");
            
            // Clean up old artifacts first
            if let Err(e) = scheduler.cleanup_old_artifacts() {
                println!("⚠️ Artifact cleanup failed: {}", e);
            }
            
            // Check for weekly report
            if scheduler.should_generate_weekly_report() {
                println!("📅 Generating weekly report...");
                match scheduler.run_dashboard_report("weekly") {
                    Ok(report_path) => {
                        println!("✅ Weekly report generated: {}", report_path);
                        if let Err(e) = scheduler.send_notifications(&report_path, "Weekly") {
                            println!("⚠️ Failed to send weekly report notifications: {}", e);
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to generate weekly report: {}", e);
                    }
                }
            }
            
            // Check for quarterly report
            if scheduler.should_generate_quarterly_report() {
                println!("📅 Generating quarterly report...");
                match scheduler.run_dashboard_report("quarterly") {
                    Ok(report_path) => {
                        println!("✅ Quarterly report generated: {}", report_path);
                        if let Err(e) = scheduler.send_notifications(&report_path, "Quarterly") {
                            println!("⚠️ Failed to send quarterly report notifications: {}", e);
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to generate quarterly report: {}", e);
                    }
                }
            }
            
            println!("✅ Automatic reporting completed");
        }
        
        "ci-integration" => {
            // CI integration mode - run gates and generate dashboard
            println!("🔗 Running CI integration mode...");
            
            match scheduler.run_integration_with_ci_gate() {
                Ok(()) => {
                    println!("✅ CI integration completed successfully");
                    std::process::exit(0);
                }
                Err(e) => {
                    println!("❌ CI integration failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        
        "weekly" => {
            // Force weekly report generation
            println!("📅 Generating weekly report (forced)...");
            match scheduler.run_dashboard_report("weekly") {
                Ok(report_path) => {
                    println!("✅ Weekly report generated: {}", report_path);
                    if let Err(e) = scheduler.send_notifications(&report_path, "Weekly") {
                        println!("⚠️ Failed to send notifications: {}", e);
                    }
                }
                Err(e) => {
                    println!("❌ Failed to generate weekly report: {}", e);
                    std::process::exit(1);
                }
            }
        }
        
        "quarterly" => {
            // Force quarterly report generation
            println!("📅 Generating quarterly report (forced)...");
            match scheduler.run_dashboard_report("quarterly") {
                Ok(report_path) => {
                    println!("✅ Quarterly report generated: {}", report_path);
                    if let Err(e) = scheduler.send_notifications(&report_path, "Quarterly") {
                        println!("⚠️ Failed to send notifications: {}", e);
                    }
                }
                Err(e) => {
                    println!("❌ Failed to generate quarterly report: {}", e);
                    std::process::exit(1);
                }
            }
        }
        
        "cleanup" => {
            // Cleanup old artifacts
            println!("🧹 Cleaning up old artifacts...");
            if let Err(e) = scheduler.cleanup_old_artifacts() {
                println!("❌ Cleanup failed: {}", e);
                std::process::exit(1);
            }
            println!("✅ Cleanup completed");
        }
        
        _ => {
            println!("❌ Invalid mode. Usage:");
            println!("  cargo +nightly -Zscript scripts/ci_dashboard_automation.rs [auto|ci-integration|weekly|quarterly|cleanup]");
            println!("");
            println!("Modes:");
            println!("  auto          - Automatic mode (checks schedule and generates reports as needed)");
            println!("  ci-integration - Run CI gates and generate dashboard report");
            println!("  weekly        - Force generate weekly report");
            println!("  quarterly     - Force generate quarterly report");
            println!("  cleanup       - Clean up old artifacts");
            println!("");
            println!("Environment Variables:");
            println!("  ENABLE_WEEKLY_REPORTS=true|false");
            println!("  ENABLE_QUARTERLY_REPORTS=true|false");
            println!("  SLACK_WEBHOOK_URL=https://hooks.slack.com/...");
            println!("  EMAIL_RECIPIENTS=user1@example.com,user2@example.com");
            println!("  ARTIFACT_RETENTION_DAYS=90");
            std::process::exit(1);
        }
    }
}