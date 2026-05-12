// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::health::HealthStream;

use super::diagnostics::{
    RuntimeHealthMessages, handle_parsed_report, record_parse_failure, record_read_failure,
};
use super::{DeviceRuntimeState, TFlightReportSource, TFlightSnapshot};

pub(super) async fn poll_device_reports(
    source: &mut dyn TFlightReportSource,
    health: &HealthStream,
    snapshots: &Arc<RwLock<HashMap<String, TFlightSnapshot>>>,
    states: &mut HashMap<String, DeviceRuntimeState>,
) {
    let paths: Vec<String> = states.keys().cloned().collect();
    for path in paths {
        let read_result = source.read_report(&path);
        let mut messages = RuntimeHealthMessages::default();
        let mut snapshot_update = None;

        if let Some(state) = states.get_mut(&path) {
            snapshot_update = process_read_result(state, read_result, &mut messages);
        }

        if let Some((snapshot_key, snapshot)) = snapshot_update {
            snapshots.write().await.insert(snapshot_key, snapshot);
        }

        messages.emit(health).await;
    }
}

fn process_read_result(
    state: &mut DeviceRuntimeState,
    read_result: Result<Option<Vec<u8>>, String>,
    messages: &mut RuntimeHealthMessages,
) -> Option<(String, TFlightSnapshot)> {
    match read_result {
        Ok(Some(report)) => match state.handler.try_parse_report(&report) {
            Ok(parsed) => {
                let snapshot = handle_parsed_report(state, parsed, messages);
                Some((state.snapshot_key.clone(), snapshot))
            }
            Err(error) => {
                record_parse_failure(state, error, messages);
                None
            }
        },
        Ok(None) => None,
        Err(error) => {
            record_read_failure(state, error, messages);
            None
        }
    }
}
