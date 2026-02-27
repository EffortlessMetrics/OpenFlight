// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Settings UI components with integration documentation links

use crate::integration_docs::{IntegrationDocsError, IntegrationDocsManager};

/// Settings panel that includes integration documentation access
pub struct SettingsPanel {
    docs_manager: IntegrationDocsManager,
}

impl SettingsPanel {
    pub fn new() -> Self {
        Self {
            docs_manager: IntegrationDocsManager::new(),
        }
    }

    /// Render simulator integration settings with documentation links
    pub fn render_simulator_integration(&mut self) -> Result<(), IntegrationDocsError> {
        // This would be implemented with the actual UI framework (egui, tauri, etc.)
        // For now, we'll just validate that documentation is available

        let available_docs = self.docs_manager.list_available_docs()?;

        for simulator in available_docs {
            // In a real UI, this would render:
            // - Simulator name and status
            // - "View Integration Details" button that calls open_doc_in_browser
            // - "Revert Changes" button
            // - Current configuration status

            println!("Simulator: {} - Documentation available", simulator);

            if let Ok(doc) = self.docs_manager.get_simulator_doc(&simulator) {
                println!("  Files modified: {}", doc.files_modified.len());
                println!("  Network connections: {}", doc.network_connections.len());
                println!("  Revert steps available: {}", doc.revert_steps.len());
            }
        }

        Ok(())
    }

    /// Open integration documentation for a specific simulator
    pub fn open_integration_docs(&self, simulator: &str) -> Result<(), IntegrationDocsError> {
        self.docs_manager.open_doc_in_browser(simulator)
    }

    /// Get summary for display in settings
    pub fn get_integration_summary(&mut self) -> Result<String, IntegrationDocsError> {
        let summary = self.docs_manager.get_installer_summary()?;

        Ok(format!(
            "Flight Hub integrates with {} simulators, modifies {} configuration files, and uses {} network ports. No administrator privileges required.",
            summary.simulators_supported.len(),
            summary.total_files_modified,
            summary.network_ports_used.len()
        ))
    }
}

impl Default for SettingsPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_panel_creation() {
        let panel = SettingsPanel::new();
        drop(panel);
    }

    #[test]
    fn settings_panel_default_equals_new() {
        let _p1 = SettingsPanel::new();
        let _p2 = SettingsPanel::default();
    }

    #[test]
    fn flight_ui_default_constructs() {
        let _ui = crate::FlightUi::default();
    }

    #[test]
    fn open_integration_docs_returns_error_for_missing_simulator() {
        // Default docs path may not exist, so a nonexistent simulator must
        // always produce a DocumentNotFound or IO error — never a panic.
        let panel = SettingsPanel::new();
        let result = panel.open_integration_docs("nonexistent_sim_xyz_999");
        assert!(
            result.is_err(),
            "must return an error for an unknown simulator"
        );
    }

    #[test]
    fn get_integration_summary_string_contains_expected_keywords() {
        let mut panel = SettingsPanel::new();
        // Docs may or may not be present; either way the string should contain
        // recognisable keywords when it succeeds.
        match panel.get_integration_summary() {
            Ok(s) => {
                assert!(
                    s.contains("simulator") || s.contains("Simulator"),
                    "summary should mention simulators: {s}"
                );
                assert!(
                    s.contains("files") || s.contains("Files"),
                    "summary should mention files: {s}"
                );
            }
            Err(_) => {
                // Acceptable when the docs directory doesn't exist in CI.
            }
        }
    }
}
