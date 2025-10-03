#!/usr/bin/env cargo +nightly -Zscript
//! Supply Chain Health Dashboard
//!
//! This script implements comprehensive metrics collection and reporting for supply chain health.
//! It provides weekly and quarterly reporting automation with trend analysis.
//!
//! Requirements addressed: Task 9.3 - Create supply chain health dashboard

use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
struct SupplyChainMetrics {
    timestamp: u64,
    gate_status: HashMap<String, bool>,
    licenses_unified_http_ok: bool,
    security_advisories_open: u32,
    license_compliance_percentage: f64,
    dependency_count: u32,
    gate_durations: HashMap<String, u64>,
    advisory_response_time_hours: Option<u64>,
}

#[derive(Debug)]
struct TrendAnalysis {
    metric_name: String,
    current_value: f64,
    previous_value: f64,
    trend_direction: TrendDirection,
    percentage_change: f64,
}

#[derive(Debug)]
enum TrendDirection {
    Improving,
    Degrading,
    Stable,
}

#[derive(Debug)]
struct DashboardReport {
    report_type: ReportType,
    generated_at: u64,
    metrics: SupplyChainMetrics,
    trends: Vec<TrendAnalysis>,
    recommendations: Vec<String>,
    slo_compliance: SloCompliance,
}

#[derive(Debug)]
enum ReportType {
    Weekly,
    Quarterly,
    OnDemand,
}

#[derive(Debug)]
struct SloCompliance {
    license_gate_duration_slo: bool,  // < 45s
    dependency_count_slo: bool,       // ≤ 150
    security_advisories_slo: bool,    // = 0
    license_completeness_slo: bool,   // = 100%
}

impl SupplyChainMetrics {
    fn collect_current() -> Result<Self, String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Collect gate status metrics
        let mut gate_status = HashMap::new();
        let mut gate_durations = HashMap::new();
        
        // Run cargo deny to check licenses gate
        let licenses_result = Self::check_licenses_gate();
        gate_status.insert("licenses".to_string(), licenses_result.0);
        gate_durations.insert("licenses".to_string(), licenses_result.1);
        
        // Run cargo audit to check security gate
        let security_result = Self::check_security_gate();
        gate_status.insert("security".to_string(), security_result.0);
        gate_durations.insert("security".to_string(), security_result.1);
        
        // Check dependencies gate
        let deps_result = Self::check_dependencies_gate();
        gate_status.insert("dependencies".to_string(), deps_result.0);
        gate_durations.insert("dependencies".to_string(), deps_result.1);
        
        // Check HTTP unification
        let http_unified = Self::check_http_unification();
        
        // Get dependency count
        let dependency_count = Self::get_dependency_count();
        
        // Get security advisories count
        let security_advisories_open = Self::get_security_advisories_count();
        
        // Calculate license compliance percentage
        let license_compliance_percentage = Self::calculate_license_compliance();
        
        // Get advisory response time (if any recent advisories)
        let advisory_response_time_hours = Self::get_advisory_response_time();
        
        Ok(SupplyChainMetrics {
            timestamp,
            gate_status,
            licenses_unified_http_ok: http_unified,
            security_advisories_open,
            license_compliance_percentage,
            dependency_count,
            gate_durations,
            advisory_response_time_hours,
        })
    }
    
    fn check_licenses_gate() -> (bool, u64) {
        let start_time = std::time::Instant::now();
        
        let output = Command::new("cargo")
            .args(&["deny", "check", "licenses", "--format", "json"])
            .output();
            
        let duration_ms = start_time.elapsed().as_millis() as u64;
        
        match output {
            Ok(result) => (result.status.success(), duration_ms),
            Err(_) => (false, duration_ms),
        }
    }
    
    fn check_security_gate() -> (bool, u64) {
        let start_time = std::time::Instant::now();
        
        let output = Command::new("cargo")
            .args(&["audit", "--deny", "warnings"])
            .output();
            
        let duration_ms = start_time.elapsed().as_millis() as u64;
        
        match output {
            Ok(result) => (result.status.success(), duration_ms),
            Err(_) => (false, duration_ms),
        }
    }
    
    fn check_dependencies_gate() -> (bool, u64) {
        let start_time = std::time::Instant::now();
        
        let count = Self::get_dependency_count();
        let duration_ms = start_time.elapsed().as_millis() as u64;
        
        (count <= 150, duration_ms)
    }
    
    fn check_http_unification() -> bool {
        // Check for absence of problematic HTTP dependencies
        let checks = vec![
            ("native-tls", false),
            ("hyper-tls", false),
            ("openssl", false),
        ];
        
        for (dep, should_exist) in checks {
            let output = Command::new("cargo")
                .args(&["tree", "-i", dep])
                .output();
                
            match output {
                Ok(result) => {
                    let has_output = !result.stdout.is_empty();
                    if has_output == should_exist {
                        continue;
                    } else {
                        return false;
                    }
                }
                Err(_) => return false,
            }
        }
        
        // Check that only hyper v1.x is present
        let output = Command::new("cargo")
            .args(&["tree", "-i", "hyper"])
            .output();
            
        match output {
            Ok(result) => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                // Check that all hyper versions are 1.x
                for line in stdout.lines() {
                    if line.contains("hyper v") {
                        if let Some(version_start) = line.find("hyper v") {
                            let version_part = &line[version_start + 7..];
                            if let Some(version_end) = version_part.find(' ') {
                                let version = &version_part[..version_end];
                                if !version.starts_with('1') {
                                    return false;
                                }
                            }
                        }
                    }
                }
                true
            }
            Err(_) => false,
        }
    }
    
    fn get_dependency_count() -> u32 {
        let output = Command::new("cargo")
            .args(&["metadata", "--format-version", "1"])
            .output();
            
        match output {
            Ok(result) if result.status.success() => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                Self::count_runtime_dependencies(&stdout).unwrap_or(0) as u32
            }
            _ => 0
        }
    }
    
    fn count_runtime_dependencies(metadata_json: &str) -> Option<usize> {
        // Simple JSON parsing to count runtime dependencies
        let mut unique_deps = std::collections::HashSet::new();
        let lines: Vec<&str> = metadata_json.lines().collect();
        
        let mut in_dependencies = false;
        let mut current_package_name = String::new();
        
        for line in lines {
            // Track package names to exclude our own crates
            if line.contains("\"name\":") && !in_dependencies {
                if let Some(name_start) = line.find("\"name\": \"") {
                    let name_part = &line[name_start + 9..];
                    if let Some(name_end) = name_part.find('"') {
                        current_package_name = name_part[..name_end].to_string();
                    }
                }
            }
            
            // Look for dependencies array
            if line.contains("\"dependencies\":") {
                in_dependencies = true;
                continue;
            }
            
            // End of dependencies array
            if in_dependencies && line.trim() == "]" {
                in_dependencies = false;
                continue;
            }
            
            // Parse dependency entries
            if in_dependencies && line.contains("\"name\":") {
                if let Some(name_start) = line.find("\"name\": \"") {
                    let name_part = &line[name_start + 9..];
                    if let Some(name_end) = name_part.find('"') {
                        let dep_name = name_part[..name_end].to_string();
                        // Only count external dependencies (not our own flight-* crates)
                        if !dep_name.starts_with("flight-") && !current_package_name.starts_with("flight-") {
                            unique_deps.insert(dep_name);
                        }
                    }
                }
            }
        }
        
        Some(unique_deps.len())
    }
    
    fn get_security_advisories_count() -> u32 {
        let output = Command::new("cargo")
            .args(&["audit", "--format", "json"])
            .output();
            
        match output {
            Ok(result) if result.status.success() => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                Self::parse_audit_advisories(&stdout).unwrap_or(0) as u32
            }
            _ => {
                // If cargo audit fails, assume there might be advisories
                // Check exit code to determine if there are actual issues
                match Command::new("cargo").args(&["audit"]).output() {
                    Ok(result) if !result.status.success() => 1, // Assume at least one advisory
                    _ => 0,
                }
            }
        }
    }
    
    fn parse_audit_advisories(json_output: &str) -> Option<usize> {
        // Simple JSON parsing to count advisories
        let mut advisory_count = 0;
        
        for line in json_output.lines() {
            if line.contains("\"advisory\"") || line.contains("\"id\":") {
                advisory_count += 1;
            }
        }
        
        Some(advisory_count / 2) // Each advisory typically has both fields
    }
    
    fn calculate_license_compliance() -> f64 {
        let output = Command::new("cargo")
            .args(&["metadata", "--format-version", "1"])
            .output();
            
        match output {
            Ok(result) if result.status.success() => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                Self::calculate_license_percentage(&stdout).unwrap_or(0.0)
            }
            _ => 0.0
        }
    }
    
    fn calculate_license_percentage(metadata_json: &str) -> Option<f64> {
        let mut total_deps = 0;
        let mut compliant_deps = 0;
        
        let lines: Vec<&str> = metadata_json.lines().collect();
        let mut in_package = false;
        let mut current_name = String::new();
        let mut current_license = None;
        
        for line in lines {
            if line.contains("\"name\":") && !in_package {
                if let Some(name_start) = line.find("\"name\": \"") {
                    let name_part = &line[name_start + 9..];
                    if let Some(name_end) = name_part.find('"') {
                        current_name = name_part[..name_end].to_string();
                        in_package = true;
                    }
                }
            }
            
            if in_package && line.contains("\"license\":") {
                if let Some(lic_start) = line.find("\"license\": \"") {
                    let lic_part = &line[lic_start + 12..];
                    if let Some(lic_end) = lic_part.find('"') {
                        current_license = Some(lic_part[..lic_end].to_string());
                    }
                }
            }
            
            if in_package && line.trim() == "}" {
                // Skip our own crates
                if !current_name.starts_with("flight-") {
                    total_deps += 1;
                    
                    if let Some(license) = &current_license {
                        if Self::is_license_compliant(license) {
                            compliant_deps += 1;
                        }
                    }
                }
                
                // Reset for next package
                in_package = false;
                current_name.clear();
                current_license = None;
            }
        }
        
        if total_deps > 0 {
            Some((compliant_deps as f64 / total_deps as f64) * 100.0)
        } else {
            Some(100.0)
        }
    }
    
    fn is_license_compliant(license: &str) -> bool {
        let approved_licenses = vec![
            "MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause",
            "Unicode-3.0", "Unicode-DFS-2016", "MPL-2.0",
            "MIT OR Apache-2.0", "Apache-2.0 OR MIT",
        ];
        
        approved_licenses.iter().any(|&approved| license.contains(approved))
    }
    
    fn get_advisory_response_time() -> Option<u64> {
        // Check for recent advisory files or CI artifacts to calculate response time
        // This is a simplified implementation - in practice, you'd track when
        // advisories were first detected vs when they were resolved
        
        if let Ok(entries) = fs::read_dir("target/ci-artifacts") {
            for entry in entries.flatten() {
                if let Some(filename) = entry.file_name().to_str() {
                    if filename.contains("audit") && filename.ends_with(".json") {
                        if let Ok(metadata) = entry.metadata() {
                            if let Ok(created) = metadata.created() {
                                let created_timestamp = created
                                    .duration_since(SystemTime::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs();
                                let now = SystemTime::now()
                                    .duration_since(SystemTime::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs();
                                
                                let hours_since = (now - created_timestamp) / 3600;
                                if hours_since < 168 { // Within last week
                                    return Some(hours_since);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        None
    }
}

impl TrendAnalysis {
    fn calculate_trends(current: &SupplyChainMetrics, historical: &[SupplyChainMetrics]) -> Vec<TrendAnalysis> {
        let mut trends = Vec::new();
        
        if let Some(previous) = historical.last() {
            // Dependency count trend
            trends.push(Self::calculate_trend(
                "dependency_count",
                current.dependency_count as f64,
                previous.dependency_count as f64,
            ));
            
            // License compliance trend
            trends.push(Self::calculate_trend(
                "license_compliance_percentage",
                current.license_compliance_percentage,
                previous.license_compliance_percentage,
            ));
            
            // Security advisories trend (lower is better)
            trends.push(Self::calculate_trend_inverted(
                "security_advisories_open",
                current.security_advisories_open as f64,
                previous.security_advisories_open as f64,
            ));
            
            // Gate duration trends
            for (gate_name, &current_duration) in &current.gate_durations {
                if let Some(&previous_duration) = previous.gate_durations.get(gate_name) {
                    trends.push(Self::calculate_trend_inverted(
                        &format!("{}_duration_ms", gate_name),
                        current_duration as f64,
                        previous_duration as f64,
                    ));
                }
            }
        }
        
        trends
    }
    
    fn calculate_trend(metric_name: &str, current: f64, previous: f64) -> TrendAnalysis {
        let percentage_change = if previous != 0.0 {
            ((current - previous) / previous) * 100.0
        } else {
            0.0
        };
        
        let trend_direction = if percentage_change.abs() < 1.0 {
            TrendDirection::Stable
        } else if percentage_change > 0.0 {
            TrendDirection::Improving
        } else {
            TrendDirection::Degrading
        };
        
        TrendAnalysis {
            metric_name: metric_name.to_string(),
            current_value: current,
            previous_value: previous,
            trend_direction,
            percentage_change,
        }
    }
    
    fn calculate_trend_inverted(metric_name: &str, current: f64, previous: f64) -> TrendAnalysis {
        let percentage_change = if previous != 0.0 {
            ((current - previous) / previous) * 100.0
        } else {
            0.0
        };
        
        let trend_direction = if percentage_change.abs() < 1.0 {
            TrendDirection::Stable
        } else if percentage_change < 0.0 {
            TrendDirection::Improving
        } else {
            TrendDirection::Degrading
        };
        
        TrendAnalysis {
            metric_name: metric_name.to_string(),
            current_value: current,
            previous_value: previous,
            trend_direction,
            percentage_change,
        }
    }
}

impl DashboardReport {
    fn generate(report_type: ReportType) -> Result<Self, String> {
        let generated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Collect current metrics
        let metrics = SupplyChainMetrics::collect_current()?;
        
        // Load historical data for trend analysis
        let historical = Self::load_historical_metrics();
        let trends = TrendAnalysis::calculate_trends(&metrics, &historical);
        
        // Generate recommendations based on current state
        let recommendations = Self::generate_recommendations(&metrics, &trends);
        
        // Calculate SLO compliance
        let slo_compliance = SloCompliance {
            license_gate_duration_slo: metrics.gate_durations
                .get("licenses")
                .map(|&duration| duration < 45000)
                .unwrap_or(false),
            dependency_count_slo: metrics.dependency_count <= 150,
            security_advisories_slo: metrics.security_advisories_open == 0,
            license_completeness_slo: metrics.license_compliance_percentage >= 100.0,
        };
        
        Ok(DashboardReport {
            report_type,
            generated_at,
            metrics,
            trends,
            recommendations,
            slo_compliance,
        })
    }
    
    fn load_historical_metrics() -> Vec<SupplyChainMetrics> {
        let mut historical = Vec::new();
        
        if let Ok(entries) = fs::read_dir("target/ci-artifacts") {
            for entry in entries.flatten() {
                if let Some(filename) = entry.file_name().to_str() {
                    if filename.starts_with("gate-metrics-") && filename.ends_with(".json") {
                        if let Ok(content) = fs::read_to_string(entry.path()) {
                            if let Ok(metrics) = Self::parse_historical_metrics(&content) {
                                historical.push(metrics);
                            }
                        }
                    }
                }
            }
        }
        
        // Sort by timestamp and keep last 12 weeks for trend analysis
        historical.sort_by_key(|m| m.timestamp);
        if historical.len() > 12 {
            historical.drain(0..historical.len() - 12);
        }
        
        historical
    }
    
    fn parse_historical_metrics(json_content: &str) -> Result<SupplyChainMetrics, String> {
        // Simple JSON parsing for historical metrics
        let gate_status = HashMap::new();
        let gate_durations = HashMap::new();
        let mut timestamp = 0;
        let mut dependency_count = 0;
        let mut security_advisories_open = 0;
        let license_compliance_percentage = 0.0;
        let mut licenses_unified_http_ok = false;
        
        for line in json_content.lines() {
            if line.contains("\"timestamp\":") {
                if let Some(ts_start) = line.find("\"timestamp\": ") {
                    let ts_part = &line[ts_start + 13..];
                    if let Some(ts_end) = ts_part.find(',') {
                        if let Ok(ts) = ts_part[..ts_end].parse::<u64>() {
                            timestamp = ts;
                        }
                    }
                }
            }
            
            if line.contains("\"scm.deps.direct_nondev_total\":") {
                if let Some(count_start) = line.find("\"scm.deps.direct_nondev_total\": ") {
                    let count_part = &line[count_start + 33..];
                    if let Some(count_end) = count_part.find(',') {
                        if let Ok(count) = count_part[..count_end].parse::<u32>() {
                            dependency_count = count;
                        }
                    }
                }
            }
            
            if line.contains("\"scm.security.advisories_open_total\":") {
                if let Some(adv_start) = line.find("\"scm.security.advisories_open_total\": ") {
                    let adv_part = &line[adv_start + 39..];
                    if let Some(adv_end) = adv_part.find(',') {
                        if let Ok(adv) = adv_part[..adv_end].parse::<u32>() {
                            security_advisories_open = adv;
                        }
                    }
                }
            }
            
            if line.contains("\"scm.licenses.unified_http_ok\":") {
                licenses_unified_http_ok = line.contains(": 1");
            }
        }
        
        Ok(SupplyChainMetrics {
            timestamp,
            gate_status,
            licenses_unified_http_ok,
            security_advisories_open,
            license_compliance_percentage,
            dependency_count,
            gate_durations,
            advisory_response_time_hours: None,
        })
    }
    
    fn generate_recommendations(metrics: &SupplyChainMetrics, trends: &[TrendAnalysis]) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        // Dependency count recommendations
        if metrics.dependency_count > 150 {
            recommendations.push(format!(
                "🚨 Dependency count ({}) exceeds threshold (150). Consider consolidating dependencies or using minimal feature sets.",
                metrics.dependency_count
            ));
        } else if metrics.dependency_count > 130 {
            recommendations.push(format!(
                "⚠️ Dependency count ({}) approaching threshold (150). Monitor new additions carefully.",
                metrics.dependency_count
            ));
        }
        
        // Security advisory recommendations
        if metrics.security_advisories_open > 0 {
            recommendations.push(format!(
                "🔒 {} open security advisories detected. Review and update affected dependencies immediately.",
                metrics.security_advisories_open
            ));
        }
        
        // License compliance recommendations
        if metrics.license_compliance_percentage < 100.0 {
            recommendations.push(format!(
                "📄 License compliance at {:.1}%. Review and approve remaining licenses or find alternatives.",
                metrics.license_compliance_percentage
            ));
        }
        
        // HTTP unification recommendations
        if !metrics.licenses_unified_http_ok {
            recommendations.push(
                "🔗 HTTP stack not unified. Ensure all HTTP dependencies use hyper 1.x and rustls-tls.".to_string()
            );
        }
        
        // Gate performance recommendations
        if let Some(license_duration) = metrics.gate_durations.get("licenses") {
            if *license_duration > 45000 {
                recommendations.push(format!(
                    "⏱️ License gate duration ({}ms) exceeds SLO (45s). Consider optimizing dependency resolution.",
                    license_duration
                ));
            }
        }
        
        // Trend-based recommendations
        for trend in trends {
            match (&trend.trend_direction, trend.metric_name.as_str()) {
                (TrendDirection::Degrading, name) if name.contains("dependency_count") => {
                    recommendations.push(format!(
                        "📈 Dependency count trending upward ({:.1}% increase). Review recent additions.",
                        trend.percentage_change.abs()
                    ));
                }
                (TrendDirection::Degrading, name) if name.contains("duration") => {
                    recommendations.push(format!(
                        "🐌 {} performance degrading ({:.1}% slower). Investigate performance bottlenecks.",
                        name, trend.percentage_change.abs()
                    ));
                }
                (TrendDirection::Degrading, name) if name.contains("security_advisories") => {
                    recommendations.push(format!(
                        "🔒 Security posture degrading. New advisories detected - prioritize updates."
                    ));
                }
                _ => {}
            }
        }
        
        // Advisory response time recommendations
        if let Some(response_time) = metrics.advisory_response_time_hours {
            if response_time > 72 {
                recommendations.push(format!(
                    "⏰ Security advisory response time ({} hours) exceeds target (72h). Improve response process.",
                    response_time
                ));
            }
        }
        
        if recommendations.is_empty() {
            recommendations.push("✅ All supply chain metrics are healthy. Continue current practices.".to_string());
        }
        
        recommendations
    }
    
    fn save_to_file(&self) -> Result<String, String> {
        let report_filename = match self.report_type {
            ReportType::Weekly => format!("supply-chain-weekly-{}.md", self.generated_at),
            ReportType::Quarterly => format!("supply-chain-quarterly-{}.md", self.generated_at),
            ReportType::OnDemand => format!("supply-chain-report-{}.md", self.generated_at),
        };
        
        let report_path = format!("target/ci-artifacts/{}", report_filename);
        
        // Ensure directory exists
        if let Err(_) = fs::create_dir_all("target/ci-artifacts") {
            return Err("Failed to create artifacts directory".to_string());
        }
        
        let report_content = self.format_markdown();
        
        fs::write(&report_path, report_content)
            .map_err(|e| format!("Failed to write report: {}", e))?;
        
        Ok(report_path)
    }
    
    fn format_markdown(&self) -> String {
        let mut content = String::new();
        
        // Header
        let report_type_str = match self.report_type {
            ReportType::Weekly => "Weekly",
            ReportType::Quarterly => "Quarterly", 
            ReportType::OnDemand => "On-Demand",
        };
        
        content.push_str(&format!("# Supply Chain Health Dashboard - {} Report\n\n", report_type_str));
        
        let date = format!("Timestamp: {}", self.generated_at);
        content.push_str(&format!("**Generated:** {}\n\n", date));
        
        // Executive Summary
        content.push_str("## Executive Summary\n\n");
        
        let overall_health = if self.slo_compliance.license_gate_duration_slo &&
                               self.slo_compliance.dependency_count_slo &&
                               self.slo_compliance.security_advisories_slo &&
                               self.slo_compliance.license_completeness_slo {
            "🟢 Healthy"
        } else if self.slo_compliance.security_advisories_slo &&
                  self.slo_compliance.license_completeness_slo {
            "🟡 Attention Needed"
        } else {
            "🔴 Critical Issues"
        };
        
        content.push_str(&format!("**Overall Health:** {}\n\n", overall_health));
        
        // Key Metrics
        content.push_str("## Key Metrics\n\n");
        content.push_str("| Metric | Current Value | SLO Target | Status |\n");
        content.push_str("|--------|---------------|------------|--------|\n");
        
        content.push_str(&format!(
            "| Dependencies (non-dev) | {} | ≤ 150 | {} |\n",
            self.metrics.dependency_count,
            if self.slo_compliance.dependency_count_slo { "✅" } else { "❌" }
        ));
        
        content.push_str(&format!(
            "| License Compliance | {:.1}% | 100% | {} |\n",
            self.metrics.license_compliance_percentage,
            if self.slo_compliance.license_completeness_slo { "✅" } else { "❌" }
        ));
        
        content.push_str(&format!(
            "| Open Security Advisories | {} | 0 | {} |\n",
            self.metrics.security_advisories_open,
            if self.slo_compliance.security_advisories_slo { "✅" } else { "❌" }
        ));
        
        if let Some(license_duration) = self.metrics.gate_durations.get("licenses") {
            content.push_str(&format!(
                "| License Gate Duration | {}ms | <45s | {} |\n",
                license_duration,
                if self.slo_compliance.license_gate_duration_slo { "✅" } else { "❌" }
            ));
        }
        
        content.push_str(&format!(
            "| HTTP Stack Unified | {} | Yes | {} |\n",
            if self.metrics.licenses_unified_http_ok { "Yes" } else { "No" },
            if self.metrics.licenses_unified_http_ok { "✅" } else { "❌" }
        ));
        
        content.push_str("\n");
        
        // Gate Status
        content.push_str("## Gate Status\n\n");
        for (gate_name, &status) in &self.metrics.gate_status {
            let status_icon = if status { "✅" } else { "❌" };
            let duration = self.metrics.gate_durations.get(gate_name)
                .map(|d| format!(" ({}ms)", d))
                .unwrap_or_default();
            content.push_str(&format!("- {} **{}**{}\n", status_icon, gate_name, duration));
        }
        content.push_str("\n");
        
        // Trend Analysis
        if !self.trends.is_empty() {
            content.push_str("## Trend Analysis\n\n");
            
            for trend in &self.trends {
                let trend_icon = match trend.trend_direction {
                    TrendDirection::Improving => "📈",
                    TrendDirection::Degrading => "📉",
                    TrendDirection::Stable => "➡️",
                };
                
                let direction_text = match trend.trend_direction {
                    TrendDirection::Improving => "improving",
                    TrendDirection::Degrading => "degrading", 
                    TrendDirection::Stable => "stable",
                };
                
                content.push_str(&format!(
                    "- {} **{}**: {:.1} → {:.1} ({:.1}% {}) {}\n",
                    trend_icon,
                    trend.metric_name.replace('_', " "),
                    trend.previous_value,
                    trend.current_value,
                    trend.percentage_change.abs(),
                    direction_text,
                    match trend.trend_direction {
                        TrendDirection::Improving => "✅",
                        TrendDirection::Degrading => "⚠️",
                        TrendDirection::Stable => "ℹ️",
                    }
                ));
            }
            content.push_str("\n");
        }
        
        // Recommendations
        content.push_str("## Recommendations\n\n");
        for (i, recommendation) in self.recommendations.iter().enumerate() {
            content.push_str(&format!("{}. {}\n", i + 1, recommendation));
        }
        content.push_str("\n");
        
        // Security Advisory Response Time
        if let Some(response_time) = self.metrics.advisory_response_time_hours {
            content.push_str("## Security Response Metrics\n\n");
            content.push_str(&format!("- **Advisory Response Time**: {} hours\n", response_time));
            content.push_str(&format!("- **Target Response Time**: 72 hours\n"));
            content.push_str(&format!("- **Status**: {}\n\n", 
                if response_time <= 72 { "✅ Within SLO" } else { "❌ Exceeds SLO" }));
        }
        
        // Footer
        content.push_str("---\n\n");
        content.push_str("*This report was automatically generated by the Supply Chain Health Dashboard.*\n");
        content.push_str("*For questions or issues, please contact the DevSecOps team.*\n");
        
        content
    }
}

fn main() {
    println!("🏥 Supply Chain Health Dashboard");
    println!("================================");
    
    let args: Vec<String> = std::env::args().collect();
    let report_type = if args.len() > 1 {
        match args[1].as_str() {
            "weekly" => ReportType::Weekly,
            "quarterly" => ReportType::Quarterly,
            _ => ReportType::OnDemand,
        }
    } else {
        ReportType::OnDemand
    };
    
    match DashboardReport::generate(report_type) {
        Ok(report) => {
            // Print summary to console
            println!("\n📊 Current Supply Chain Health:");
            println!("  Dependencies: {} (target: ≤150)", report.metrics.dependency_count);
            println!("  License Compliance: {:.1}% (target: 100%)", report.metrics.license_compliance_percentage);
            println!("  Security Advisories: {} (target: 0)", report.metrics.security_advisories_open);
            println!("  HTTP Stack Unified: {}", if report.metrics.licenses_unified_http_ok { "Yes ✅" } else { "No ❌" });
            
            // Show gate performance
            if let Some(license_duration) = report.metrics.gate_durations.get("licenses") {
                let slo_status = if *license_duration < 45000 { "✅" } else { "❌" };
                println!("  License Gate: {}ms (SLO: <45s) {}", license_duration, slo_status);
            }
            
            // Show trends
            if !report.trends.is_empty() {
                println!("\n📈 Key Trends:");
                for trend in &report.trends {
                    let icon = match trend.trend_direction {
                        TrendDirection::Improving => "📈",
                        TrendDirection::Degrading => "📉", 
                        TrendDirection::Stable => "➡️",
                    };
                    println!("  {} {}: {:.1}% change", icon, trend.metric_name.replace('_', " "), trend.percentage_change);
                }
            }
            
            // Show top recommendations
            if !report.recommendations.is_empty() {
                println!("\n💡 Top Recommendations:");
                for (i, rec) in report.recommendations.iter().take(3).enumerate() {
                    println!("  {}. {}", i + 1, rec);
                }
            }
            
            // Save detailed report
            match report.save_to_file() {
                Ok(path) => {
                    println!("\n📄 Detailed report saved to: {}", path);
                    
                    // Also save metrics in JSON format for automation
                    let json_path = path.replace(".md", ".json");
                    let json_content = format!(
                        r#"{{
  "timestamp": {},
  "report_type": "{:?}",
  "metrics": {{
    "dependency_count": {},
    "license_compliance_percentage": {},
    "security_advisories_open": {},
    "licenses_unified_http_ok": {},
    "gate_durations": {{{}}}
  }},
  "slo_compliance": {{
    "license_gate_duration_slo": {},
    "dependency_count_slo": {},
    "security_advisories_slo": {},
    "license_completeness_slo": {}
  }},
  "recommendations_count": {}
}}"#,
                        report.generated_at,
                        report.report_type,
                        report.metrics.dependency_count,
                        report.metrics.license_compliance_percentage,
                        report.metrics.security_advisories_open,
                        report.metrics.licenses_unified_http_ok,
                        report.metrics.gate_durations.iter()
                            .map(|(k, v)| format!("\"{}\":{}", k, v))
                            .collect::<Vec<_>>()
                            .join(","),
                        report.slo_compliance.license_gate_duration_slo,
                        report.slo_compliance.dependency_count_slo,
                        report.slo_compliance.security_advisories_slo,
                        report.slo_compliance.license_completeness_slo,
                        report.recommendations.len()
                    );
                    
                    if let Err(e) = fs::write(&json_path, json_content) {
                        println!("⚠️ Failed to save JSON metrics: {}", e);
                    } else {
                        println!("📊 JSON metrics saved to: {}", json_path);
                    }
                }
                Err(e) => {
                    println!("❌ Failed to save report: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to generate dashboard report: {}", e);
            std::process::exit(1);
        }
    }
    
    println!("\n✅ Supply Chain Health Dashboard completed successfully!");
}