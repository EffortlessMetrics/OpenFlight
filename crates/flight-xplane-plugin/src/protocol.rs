// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Newline-delimited JSON protocol shared between Flight Hub and the X-Plane plugin.
//!
//! These types mirror `flight-xplane::plugin::{PluginMessage, PluginResponse}`.
//! They are intentionally re-defined here so the plugin crate can be built
//! independently without pulling in the full flight-xplane dependency tree.

use serde::{Deserialize, Serialize};

/// Messages sent from Flight Hub → plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginMessage {
    Handshake {
        version: String,
        capabilities: Vec<String>,
    },
    GetDataRef {
        id: u32,
        name: String,
    },
    SetDataRef {
        id: u32,
        name: String,
        value: serde_json::Value,
    },
    Subscribe {
        id: u32,
        name: String,
        frequency: f32,
    },
    Unsubscribe {
        id: u32,
        name: String,
    },
    Command {
        id: u32,
        name: String,
    },
    GetAircraftInfo {
        id: u32,
    },
    Ping {
        id: u32,
        timestamp: u64,
    },
}

/// Messages sent from plugin → Flight Hub.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginResponse {
    HandshakeAck {
        version: String,
        capabilities: Vec<String>,
        status: String,
    },
    DataRefValue {
        id: u32,
        name: String,
        value: serde_json::Value,
        timestamp: u64,
    },
    DataRefUpdate {
        name: String,
        value: serde_json::Value,
        timestamp: u64,
    },
    CommandResult {
        id: u32,
        success: bool,
        message: Option<String>,
    },
    AircraftInfo {
        id: u32,
        icao: String,
        title: String,
        author: String,
        file_path: String,
    },
    Error {
        id: Option<u32>,
        error: String,
        details: Option<String>,
    },
    Pong {
        id: u32,
        timestamp: u64,
    },
}
