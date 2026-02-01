// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Product posture CLI command
//!
//! Displays the product posture summary during installation.

use crate::client_manager::ClientManager;
use crate::output::OutputFormat;
use anyhow::Result;
use serde_json::json;

/// Product posture text displayed during installation
const PRODUCT_POSTURE: &str = r#"
╔══════════════════════════════════════════════════════════════════════════════╗
║                           FLIGHT HUB - PRODUCT POSTURE                       ║
╠══════════════════════════════════════════════════════════════════════════════╣
║                                                                              ║
║  Flight Hub is an INPUT MANAGEMENT ACCESSORY for flight simulation.         ║
║                                                                              ║
║  WHAT FLIGHT HUB IS:                                                         ║
║  • A unified control plane for flight controls, panels, and FFB devices     ║
║  • An input processor that enhances your hardware experience                 ║
║  • A bridge between your hardware and supported flight simulators           ║
║                                                                              ║
║  WHAT FLIGHT HUB IS NOT:                                                     ║
║  • Not a game or simulator itself                                            ║
║  • Not affiliated with any simulator vendor                                  ║
║  • Not a replacement for simulator functionality                             ║
║                                                                              ║
║  IMPORTANT NOTICES:                                                          ║
║  • Flight Hub operates locally on your PC only                               ║
║  • No data is transmitted to external servers                                ║
║  • Simulator integrations are optional and reversible                        ║
║  • Please review each simulator's EULA for third-party tool policies        ║
║                                                                              ║
║  SUPPORTED SIMULATORS:                                                       ║
║  • Microsoft Flight Simulator (MSFS 2020/2024)                              ║
║  • X-Plane 11/12                                                             ║
║  • DCS World                                                                 ║
║                                                                              ║
║  For more information, see: docs/product-posture.md                         ║
║                                                                              ║
╚══════════════════════════════════════════════════════════════════════════════╝
"#;

pub async fn execute(
    output_format: OutputFormat,
    _verbose: bool,
    _client_manager: &ClientManager,
) -> Result<Option<String>> {
    let output = match output_format {
        OutputFormat::Json => json!({
            "product_type": "input_management_accessory",
            "description": "Unified control plane for flight controls, panels, and FFB devices",
            "local_only": true,
            "supported_simulators": ["MSFS", "X-Plane", "DCS"],
            "posture_text": PRODUCT_POSTURE.trim()
        })
        .to_string(),
        OutputFormat::Human => PRODUCT_POSTURE.trim().to_string(),
    };

    Ok(Some(output))
}
