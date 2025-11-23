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
        // Basic test to ensure the panel can be created
        assert!(true);
    }
}
