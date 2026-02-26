// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for JSON patch (RFC 6902) operations in flight-writers.
//!
//! Run with: `cargo +nightly fuzz run fuzz_json_patch`

#![no_main]

use flight_writers::{JsonPatchOp, JsonPatchOpType};
use libfuzzer_sys::fuzz_target;
use serde_json::Value;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Try to deserialize as a JSON patch operation array — must never panic
    if let Ok(patches) = serde_json::from_str::<Vec<JsonPatchOp>>(input) {
        let mut doc: Value = serde_json::json!({
            "a": 1,
            "b": { "c": 2 },
            "arr": [1, 2, 3]
        });

        // Apply each patch — errors are expected for malformed input
        for patch in &patches {
            let _ = apply_single_patch(&mut doc, patch);
        }
    }

    // Also try fuzzing direct JSON document parsing
    if let Ok(doc) = serde_json::from_str::<Value>(input) {
        // Try parsing as a remove patch targeting the document
        let patch = JsonPatchOp {
            op: JsonPatchOpType::Test,
            path: "/a".to_string(),
            value: Some(doc.clone()),
            from: None,
        };
        let mut target = serde_json::json!({"a": doc});
        let _ = apply_single_patch(&mut target, &patch);
    }
});

/// Apply a single patch operation to a JSON document, returning any error.
fn apply_single_patch(doc: &mut Value, patch: &JsonPatchOp) -> Result<(), String> {
    let path_parts = parse_json_pointer(&patch.path);

    match patch.op {
        JsonPatchOpType::Add => {
            set_value_at_path(doc, &path_parts, patch.value.clone().unwrap_or(Value::Null));
            Ok(())
        }
        JsonPatchOpType::Remove => {
            remove_value_at_path(doc, &path_parts)?;
            Ok(())
        }
        JsonPatchOpType::Replace => {
            let val = patch.value.clone().unwrap_or(Value::Null);
            set_value_at_path(doc, &path_parts, val);
            Ok(())
        }
        JsonPatchOpType::Test => {
            let actual = get_value_at_path(doc, &path_parts);
            let expected = patch.value.as_ref();
            if actual.is_some() == expected.is_some() {
                Ok(())
            } else {
                Err("test failed".to_string())
            }
        }
        JsonPatchOpType::Move | JsonPatchOpType::Copy => {
            let from = patch.from.as_deref().unwrap_or("");
            let from_parts = parse_json_pointer(from);
            if let Some(val) = get_value_at_path(doc, &from_parts).cloned() {
                set_value_at_path(doc, &path_parts, val);
                if matches!(patch.op, JsonPatchOpType::Move) {
                    let _ = remove_value_at_path(doc, &from_parts);
                }
                Ok(())
            } else {
                Err("source not found".to_string())
            }
        }
    }
}

fn parse_json_pointer(ptr: &str) -> Vec<String> {
    if ptr.is_empty() || ptr == "/" {
        return vec![];
    }
    ptr.trim_start_matches('/')
        .split('/')
        .map(|s| s.replace("~1", "/").replace("~0", "~"))
        .collect()
}

fn get_value_at_path<'a>(doc: &'a Value, parts: &[String]) -> Option<&'a Value> {
    let mut cur = doc;
    for part in parts {
        cur = match cur {
            Value::Object(map) => map.get(part.as_str())?,
            Value::Array(arr) => {
                let idx: usize = part.parse().ok()?;
                arr.get(idx)?
            }
            _ => return None,
        };
    }
    Some(cur)
}

fn set_value_at_path(doc: &mut Value, parts: &[String], val: Value) {
    if parts.is_empty() {
        *doc = val;
        return;
    }
    let mut cur = doc;
    for part in parts.iter().take(parts.len().saturating_sub(1)) {
        cur = match cur {
            Value::Object(map) => map.entry(part.as_str()).or_insert(Value::Object(Default::default())),
            _ => return,
        };
    }
    if let Some(last) = parts.last() {
        if let Value::Object(map) = cur {
            map.insert(last.clone(), val);
        }
    }
}

fn remove_value_at_path(doc: &mut Value, parts: &[String]) -> Result<(), String> {
    if parts.is_empty() {
        return Err("cannot remove root".to_string());
    }
    let mut cur = doc;
    for part in parts.iter().take(parts.len().saturating_sub(1)) {
        cur = match cur {
            Value::Object(map) => map.get_mut(part.as_str()).ok_or("path not found")?,
            Value::Array(arr) => {
                let idx: usize = part.parse().map_err(|_| "invalid index")?;
                arr.get_mut(idx).ok_or("index out of bounds")?
            }
            _ => return Err("not an object/array".to_string()),
        };
    }
    if let Some(last) = parts.last() {
        match cur {
            Value::Object(map) => {
                map.remove(last.as_str());
                Ok(())
            }
            Value::Array(arr) => {
                let idx: usize = last.parse().map_err(|_| "invalid index")?;
                if idx < arr.len() {
                    arr.remove(idx);
                }
                Ok(())
            }
            _ => Err("not an object/array".to_string()),
        }
    } else {
        Err("empty path".to_string())
    }
}
