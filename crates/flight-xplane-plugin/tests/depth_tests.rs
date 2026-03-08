// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the flight-xplane-plugin crate.
//!
//! Covers DataRef management, command handling, plugin protocol,
//! bridge message processing, and property-based invariants.

use flight_xplane_plugin::bridge::handle_message;
use flight_xplane_plugin::protocol::{PluginMessage, PluginResponse};
use flight_xplane_plugin::xplm::{
    self, MockValue, clear_mocks, execute_command, executed_commands, read_dataref,
    read_dataref_string, set_mock_dataref, timestamp_ms, write_dataref,
};
use proptest::prelude::*;

// ── DataRef registration & lookup ──────────────────────────────────────────

#[test]
fn dataref_float_round_trip() {
    clear_mocks();
    set_mock_dataref("sim/test/f", MockValue::Float(1.5));
    let val = read_dataref("sim/test/f").unwrap();
    assert!((val.as_f64().unwrap() - 1.5).abs() < 0.01);
}

#[test]
fn dataref_int_round_trip() {
    clear_mocks();
    set_mock_dataref("sim/test/i", MockValue::Int(42));
    let val = read_dataref("sim/test/i").unwrap();
    assert_eq!(val.as_i64().unwrap(), 42);
}

#[test]
fn dataref_double_round_trip() {
    clear_mocks();
    set_mock_dataref("sim/test/d", MockValue::Double(std::f64::consts::PI));
    let val = read_dataref("sim/test/d").unwrap();
    assert!((val.as_f64().unwrap() - std::f64::consts::PI).abs() < 1e-10);
}

#[test]
fn dataref_data_returns_json_string() {
    clear_mocks();
    set_mock_dataref("sim/test/data", MockValue::Data(b"hello\0".to_vec()));
    let val = read_dataref("sim/test/data").unwrap();
    assert!(val.as_str().unwrap().starts_with("hello"));
}

#[test]
fn dataref_not_found_returns_none() {
    clear_mocks();
    assert!(read_dataref("sim/does/not/exist").is_none());
}

#[test]
fn dataref_overwrite_replaces_value() {
    clear_mocks();
    set_mock_dataref("sim/test/ow", MockValue::Float(1.0));
    set_mock_dataref("sim/test/ow", MockValue::Float(2.0));
    let val = read_dataref("sim/test/ow").unwrap();
    assert!((val.as_f64().unwrap() - 2.0).abs() < 0.01);
}

#[test]
fn dataref_clear_removes_all() {
    set_mock_dataref("sim/test/a", MockValue::Int(1));
    set_mock_dataref("sim/test/b", MockValue::Int(2));
    clear_mocks();
    assert!(read_dataref("sim/test/a").is_none());
    assert!(read_dataref("sim/test/b").is_none());
}

#[test]
fn dataref_multiple_types_coexist() {
    clear_mocks();
    set_mock_dataref("sim/test/float", MockValue::Float(1.0));
    set_mock_dataref("sim/test/int", MockValue::Int(2));
    set_mock_dataref("sim/test/double", MockValue::Double(3.0));
    set_mock_dataref("sim/test/data", MockValue::Data(b"abc\0".to_vec()));

    assert!(read_dataref("sim/test/float").unwrap().is_f64());
    assert!(read_dataref("sim/test/int").unwrap().is_i64());
    assert!(read_dataref("sim/test/double").unwrap().is_f64());
    assert!(read_dataref("sim/test/data").unwrap().is_string());
}

// ── DataRef string access ──────────────────────────────────────────────────

#[test]
fn dataref_string_trims_nul_terminator() {
    clear_mocks();
    set_mock_dataref("sim/test/str", MockValue::Data(b"B738\0\0\0".to_vec()));
    assert_eq!(read_dataref_string("sim/test/str").unwrap(), "B738");
}

#[test]
fn dataref_string_without_nul_returns_full() {
    clear_mocks();
    set_mock_dataref("sim/test/str2", MockValue::Data(b"ABCD".to_vec()));
    assert_eq!(read_dataref_string("sim/test/str2").unwrap(), "ABCD");
}

#[test]
fn dataref_string_empty_bytes() {
    clear_mocks();
    set_mock_dataref("sim/test/empty", MockValue::Data(b"\0".to_vec()));
    assert_eq!(read_dataref_string("sim/test/empty").unwrap(), "");
}

#[test]
fn dataref_string_returns_none_for_float() {
    clear_mocks();
    set_mock_dataref("sim/test/wrongtype", MockValue::Float(1.0));
    assert!(read_dataref_string("sim/test/wrongtype").is_none());
}

#[test]
fn dataref_string_returns_none_when_missing() {
    clear_mocks();
    assert!(read_dataref_string("sim/nonexistent").is_none());
}

// ── Write DataRef ──────────────────────────────────────────────────────────

#[test]
fn write_dataref_float_updates_value() {
    clear_mocks();
    set_mock_dataref("sim/test/w", MockValue::Float(0.0));
    assert!(write_dataref("sim/test/w", &serde_json::json!(42.5)));
    let val = read_dataref("sim/test/w").unwrap();
    assert!((val.as_f64().unwrap() - 42.5).abs() < 0.1);
}

#[test]
fn write_dataref_int_via_json_number() {
    clear_mocks();
    set_mock_dataref("sim/test/wi", MockValue::Int(0));
    assert!(write_dataref("sim/test/wi", &serde_json::json!(100)));
    let val = read_dataref("sim/test/wi").unwrap();
    // Written as float since JSON numbers are f64 first
    assert!(val.as_f64().unwrap().abs() - 100.0 < 0.1);
}

#[test]
fn write_dataref_fails_when_not_found() {
    clear_mocks();
    assert!(!write_dataref("sim/missing", &serde_json::json!(1.0)));
}

#[test]
fn write_dataref_rejects_string_value() {
    clear_mocks();
    set_mock_dataref("sim/test/ns", MockValue::Float(0.0));
    assert!(!write_dataref(
        "sim/test/ns",
        &serde_json::json!("not a number")
    ));
}

#[test]
fn write_dataref_rejects_null_value() {
    clear_mocks();
    set_mock_dataref("sim/test/null", MockValue::Float(0.0));
    assert!(!write_dataref("sim/test/null", &serde_json::Value::Null));
}

#[test]
fn write_dataref_rejects_object_value() {
    clear_mocks();
    set_mock_dataref("sim/test/obj", MockValue::Float(0.0));
    assert!(!write_dataref(
        "sim/test/obj",
        &serde_json::json!({"key": "val"})
    ));
}

#[test]
fn write_then_read_preserves_approximate_value() {
    clear_mocks();
    set_mock_dataref("sim/test/wr", MockValue::Float(0.0));
    write_dataref("sim/test/wr", &serde_json::json!(7.77));
    let val = read_dataref("sim/test/wr").unwrap().as_f64().unwrap();
    assert!((val - 7.77).abs() < 0.01);
}

// ── Command execution ──────────────────────────────────────────────────────

#[test]
fn command_execute_records() {
    clear_mocks();
    assert!(execute_command("sim/autopilot/heading_sync"));
    assert_eq!(executed_commands(), vec!["sim/autopilot/heading_sync"]);
}

#[test]
fn command_execute_multiple_in_order() {
    clear_mocks();
    execute_command("sim/cmd/a");
    execute_command("sim/cmd/b");
    execute_command("sim/cmd/c");
    assert_eq!(
        executed_commands(),
        vec!["sim/cmd/a", "sim/cmd/b", "sim/cmd/c"]
    );
}

#[test]
fn command_execute_same_twice() {
    clear_mocks();
    execute_command("sim/cmd/dup");
    execute_command("sim/cmd/dup");
    assert_eq!(executed_commands(), vec!["sim/cmd/dup", "sim/cmd/dup"]);
}

#[test]
fn clear_mocks_resets_commands() {
    execute_command("sim/cmd/before");
    clear_mocks();
    assert!(executed_commands().is_empty());
}

// ── Timestamp ──────────────────────────────────────────────────────────────

#[test]
fn timestamp_is_nonzero() {
    assert!(timestamp_ms() > 0);
}

#[test]
fn timestamp_is_monotonic() {
    let t1 = timestamp_ms();
    let t2 = timestamp_ms();
    assert!(t2 >= t1);
}

// ── Bridge: handle_message (GetDataRef) ────────────────────────────────────

#[test]
fn bridge_get_dataref_returns_float() {
    clear_mocks();
    set_mock_dataref("sim/cockpit/ap/alt", MockValue::Float(35_000.0));
    let msg = PluginMessage::GetDataRef {
        id: 1,
        name: "sim/cockpit/ap/alt".to_string(),
    };
    match handle_message(msg).unwrap() {
        PluginResponse::DataRefValue {
            id, name, value, ..
        } => {
            assert_eq!(id, 1);
            assert_eq!(name, "sim/cockpit/ap/alt");
            assert!((value.as_f64().unwrap() - 35_000.0).abs() < 1.0);
        }
        other => panic!("Expected DataRefValue, got {other:?}"),
    }
}

#[test]
fn bridge_get_dataref_returns_int() {
    clear_mocks();
    set_mock_dataref("sim/test/gear", MockValue::Int(1));
    let msg = PluginMessage::GetDataRef {
        id: 10,
        name: "sim/test/gear".to_string(),
    };
    match handle_message(msg).unwrap() {
        PluginResponse::DataRefValue { value, .. } => {
            assert_eq!(value.as_i64().unwrap(), 1);
        }
        other => panic!("Expected DataRefValue, got {other:?}"),
    }
}

#[test]
fn bridge_get_dataref_returns_double() {
    clear_mocks();
    set_mock_dataref("sim/test/lat", MockValue::Double(47.6062));
    let msg = PluginMessage::GetDataRef {
        id: 11,
        name: "sim/test/lat".to_string(),
    };
    match handle_message(msg).unwrap() {
        PluginResponse::DataRefValue { value, .. } => {
            assert!((value.as_f64().unwrap() - 47.6062).abs() < 1e-4);
        }
        other => panic!("Expected DataRefValue, got {other:?}"),
    }
}

#[test]
fn bridge_get_dataref_error_when_missing() {
    clear_mocks();
    let msg = PluginMessage::GetDataRef {
        id: 2,
        name: "sim/missing".to_string(),
    };
    match handle_message(msg).unwrap() {
        PluginResponse::Error { id, error, .. } => {
            assert_eq!(id, Some(2));
            assert!(error.contains("not found"));
        }
        other => panic!("Expected Error, got {other:?}"),
    }
}

#[test]
fn bridge_get_dataref_includes_timestamp() {
    clear_mocks();
    set_mock_dataref("sim/test/ts", MockValue::Float(1.0));
    let msg = PluginMessage::GetDataRef {
        id: 20,
        name: "sim/test/ts".to_string(),
    };
    if let PluginResponse::DataRefValue { timestamp, .. } = handle_message(msg).unwrap() {
        assert!(timestamp > 0);
    } else {
        panic!("Expected DataRefValue");
    }
}

// ── Bridge: handle_message (SetDataRef) ────────────────────────────────────

#[test]
fn bridge_set_dataref_success() {
    clear_mocks();
    set_mock_dataref("sim/test/set", MockValue::Float(0.0));
    let msg = PluginMessage::SetDataRef {
        id: 3,
        name: "sim/test/set".to_string(),
        value: serde_json::json!(99.0),
    };
    match handle_message(msg).unwrap() {
        PluginResponse::CommandResult {
            id,
            success,
            message,
        } => {
            assert_eq!(id, 3);
            assert!(success);
            assert!(message.is_none());
        }
        other => panic!("Expected CommandResult, got {other:?}"),
    }
}

#[test]
fn bridge_set_dataref_failure_missing() {
    clear_mocks();
    let msg = PluginMessage::SetDataRef {
        id: 4,
        name: "sim/nonexistent".to_string(),
        value: serde_json::json!(1.0),
    };
    match handle_message(msg).unwrap() {
        PluginResponse::CommandResult {
            id,
            success,
            message,
        } => {
            assert_eq!(id, 4);
            assert!(!success);
            assert!(message.is_some());
        }
        other => panic!("Expected CommandResult, got {other:?}"),
    }
}

// ── Bridge: handle_message (Command) ───────────────────────────────────────

#[test]
fn bridge_command_executes() {
    clear_mocks();
    let msg = PluginMessage::Command {
        id: 5,
        name: "sim/autopilot/engage".to_string(),
    };
    match handle_message(msg).unwrap() {
        PluginResponse::CommandResult {
            id,
            success,
            message,
        } => {
            assert_eq!(id, 5);
            assert!(success);
            assert!(message.is_none());
        }
        other => panic!("Expected CommandResult, got {other:?}"),
    }
    assert_eq!(executed_commands(), vec!["sim/autopilot/engage"]);
}

// ── Bridge: handle_message (Ping) ──────────────────────────────────────────

#[test]
fn bridge_ping_returns_pong_with_same_id_and_timestamp() {
    let msg = PluginMessage::Ping {
        id: 100,
        timestamp: 9876543210,
    };
    match handle_message(msg).unwrap() {
        PluginResponse::Pong { id, timestamp } => {
            assert_eq!(id, 100);
            assert_eq!(timestamp, 9876543210);
        }
        other => panic!("Expected Pong, got {other:?}"),
    }
}

// ── Bridge: handle_message (GetAircraftInfo) ───────────────────────────────

#[test]
fn bridge_aircraft_info_populated() {
    clear_mocks();
    set_mock_dataref(
        "sim/aircraft/view/acf_ICAO",
        MockValue::Data(b"A320\0".to_vec()),
    );
    set_mock_dataref(
        "sim/aircraft/view/acf_descrip",
        MockValue::Data(b"Airbus A320neo\0".to_vec()),
    );
    set_mock_dataref(
        "sim/aircraft/view/acf_author",
        MockValue::Data(b"FlightFactor\0".to_vec()),
    );
    set_mock_dataref(
        "sim/aircraft/view/acf_relative_path",
        MockValue::Data(b"Aircraft/A320/A320.acf\0".to_vec()),
    );
    let msg = PluginMessage::GetAircraftInfo { id: 50 };
    match handle_message(msg).unwrap() {
        PluginResponse::AircraftInfo {
            id,
            icao,
            title,
            author,
            file_path,
        } => {
            assert_eq!(id, 50);
            assert_eq!(icao, "A320");
            assert_eq!(title, "Airbus A320neo");
            assert_eq!(author, "FlightFactor");
            assert_eq!(file_path, "Aircraft/A320/A320.acf");
        }
        other => panic!("Expected AircraftInfo, got {other:?}"),
    }
}

#[test]
fn bridge_aircraft_info_defaults_when_empty() {
    clear_mocks();
    let msg = PluginMessage::GetAircraftInfo { id: 51 };
    match handle_message(msg).unwrap() {
        PluginResponse::AircraftInfo {
            id,
            icao,
            title,
            author,
            file_path,
        } => {
            assert_eq!(id, 51);
            assert_eq!(icao, "UNKN");
            assert!(title.is_empty());
            assert!(author.is_empty());
            assert!(file_path.is_empty());
        }
        other => panic!("Expected AircraftInfo, got {other:?}"),
    }
}

// ── Bridge: non-responding messages ────────────────────────────────────────

#[test]
fn bridge_handshake_returns_none() {
    let msg = PluginMessage::Handshake {
        version: "1.0".to_string(),
        capabilities: vec!["subscribe".to_string()],
    };
    assert!(handle_message(msg).is_none());
}

#[test]
fn bridge_subscribe_returns_none() {
    let msg = PluginMessage::Subscribe {
        id: 60,
        name: "sim/test/sub".to_string(),
        frequency: 10.0,
    };
    assert!(handle_message(msg).is_none());
}

#[test]
fn bridge_unsubscribe_returns_none() {
    let msg = PluginMessage::Unsubscribe {
        id: 61,
        name: "sim/test/sub".to_string(),
    };
    assert!(handle_message(msg).is_none());
}

// ── Protocol serialization ─────────────────────────────────────────────────

#[test]
fn protocol_all_message_variants_serialize() {
    let messages: Vec<PluginMessage> = vec![
        PluginMessage::Handshake {
            version: "1.0".to_string(),
            capabilities: vec![],
        },
        PluginMessage::GetDataRef {
            id: 1,
            name: "sim/test".to_string(),
        },
        PluginMessage::SetDataRef {
            id: 2,
            name: "sim/test".to_string(),
            value: serde_json::json!(1.0),
        },
        PluginMessage::Subscribe {
            id: 3,
            name: "sim/test".to_string(),
            frequency: 10.0,
        },
        PluginMessage::Unsubscribe {
            id: 4,
            name: "sim/test".to_string(),
        },
        PluginMessage::Command {
            id: 5,
            name: "sim/cmd".to_string(),
        },
        PluginMessage::GetAircraftInfo { id: 6 },
        PluginMessage::Ping {
            id: 7,
            timestamp: 123,
        },
    ];

    for msg in &messages {
        let json = serde_json::to_string(msg).unwrap();
        let decoded: PluginMessage = serde_json::from_str(&json).unwrap();
        // Verify type tag survives round-trip
        let json2 = serde_json::to_string(&decoded).unwrap();
        assert_eq!(json, json2);
    }
}

#[test]
fn protocol_all_response_variants_serialize() {
    let responses: Vec<PluginResponse> = vec![
        PluginResponse::HandshakeAck {
            version: "1.0".to_string(),
            capabilities: vec!["read_datarefs".to_string()],
            status: "ready".to_string(),
        },
        PluginResponse::DataRefValue {
            id: 1,
            name: "sim/test".to_string(),
            value: serde_json::json!(42.0),
            timestamp: 1000,
        },
        PluginResponse::DataRefUpdate {
            name: "sim/test".to_string(),
            value: serde_json::json!(43.0),
            timestamp: 1001,
        },
        PluginResponse::CommandResult {
            id: 2,
            success: true,
            message: None,
        },
        PluginResponse::AircraftInfo {
            id: 3,
            icao: "B738".to_string(),
            title: "Boeing 737-800".to_string(),
            author: "Zibo".to_string(),
            file_path: "Aircraft/B738.acf".to_string(),
        },
        PluginResponse::Error {
            id: Some(4),
            error: "not found".to_string(),
            details: Some("detail".to_string()),
        },
        PluginResponse::Pong {
            id: 5,
            timestamp: 9999,
        },
    ];

    for resp in &responses {
        let json = serde_json::to_string(resp).unwrap();
        let decoded: PluginResponse = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&decoded).unwrap();
        assert_eq!(json, json2);
    }
}

#[test]
fn protocol_malformed_json_is_error() {
    assert!(serde_json::from_str::<PluginMessage>("{bad}").is_err());
}

#[test]
fn protocol_unknown_type_tag_is_error() {
    assert!(
        serde_json::from_str::<PluginMessage>(r#"{"type":"FooBar","id":1}"#).is_err()
    );
}

#[test]
fn protocol_missing_required_fields_is_error() {
    // GetDataRef requires both id and name
    assert!(serde_json::from_str::<PluginMessage>(r#"{"type":"GetDataRef","id":1}"#).is_err());
}

#[test]
fn protocol_error_response_with_none_id() {
    let resp = PluginResponse::Error {
        id: None,
        error: "generic".to_string(),
        details: None,
    };
    let json = serde_json::to_string(&resp).unwrap();
    let decoded: PluginResponse = serde_json::from_str(&json).unwrap();
    if let PluginResponse::Error { id, .. } = decoded {
        assert!(id.is_none());
    } else {
        panic!("Wrong variant");
    }
}

#[test]
fn protocol_error_response_with_details() {
    let resp = PluginResponse::Error {
        id: Some(99),
        error: "failed".to_string(),
        details: Some("stack trace here".to_string()),
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("stack trace here"));
}

#[test]
fn protocol_subscribe_includes_frequency() {
    let msg = PluginMessage::Subscribe {
        id: 10,
        name: "sim/test".to_string(),
        frequency: 20.0,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("20.0") || json.contains("20"));
}

#[test]
fn protocol_dataref_update_has_no_id() {
    let resp = PluginResponse::DataRefUpdate {
        name: "sim/test".to_string(),
        value: serde_json::json!(1.0),
        timestamp: 500,
    };
    let json = serde_json::to_string(&resp).unwrap();
    // DataRefUpdate should not have an "id" field
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.get("id").is_none());
}

// ── XPLM type constants ───────────────────────────────────────────────────

#[test]
fn xplm_type_constants_are_powers_of_two() {
    assert_eq!(xplm::XPLM_TYPE_INT, 1);
    assert_eq!(xplm::XPLM_TYPE_FLOAT, 2);
    assert_eq!(xplm::XPLM_TYPE_DOUBLE, 4);
    assert_eq!(xplm::XPLM_TYPE_FLOAT_ARRAY, 8);
    assert_eq!(xplm::XPLM_TYPE_INT_ARRAY, 16);
    assert_eq!(xplm::XPLM_TYPE_DATA, 32);
}

#[test]
fn xplm_type_flags_are_orthogonal() {
    let all = [
        xplm::XPLM_TYPE_INT,
        xplm::XPLM_TYPE_FLOAT,
        xplm::XPLM_TYPE_DOUBLE,
        xplm::XPLM_TYPE_FLOAT_ARRAY,
        xplm::XPLM_TYPE_INT_ARRAY,
        xplm::XPLM_TYPE_DATA,
    ];
    for (i, &a) in all.iter().enumerate() {
        for (j, &b) in all.iter().enumerate() {
            if i != j {
                assert_eq!(a & b, 0, "flags {a} and {b} overlap");
            }
        }
    }
}

// ── Edge cases ─────────────────────────────────────────────────────────────

#[test]
fn dataref_empty_name() {
    clear_mocks();
    assert!(read_dataref("").is_none());
}

#[test]
fn dataref_very_long_name() {
    clear_mocks();
    let long_name = "sim/".to_string() + &"x".repeat(1000);
    set_mock_dataref(&long_name, MockValue::Int(1));
    assert_eq!(read_dataref(&long_name).unwrap().as_i64().unwrap(), 1);
}

#[test]
fn dataref_with_special_characters() {
    clear_mocks();
    set_mock_dataref("sim/test/with spaces", MockValue::Int(5));
    assert_eq!(
        read_dataref("sim/test/with spaces")
            .unwrap()
            .as_i64()
            .unwrap(),
        5
    );
}

#[test]
fn write_dataref_negative_float() {
    clear_mocks();
    set_mock_dataref("sim/test/neg", MockValue::Float(0.0));
    assert!(write_dataref("sim/test/neg", &serde_json::json!(-500.5)));
    let val = read_dataref("sim/test/neg").unwrap().as_f64().unwrap();
    assert!((val - (-500.5)).abs() < 0.1);
}

#[test]
fn write_dataref_zero() {
    clear_mocks();
    set_mock_dataref("sim/test/zero", MockValue::Float(99.0));
    assert!(write_dataref("sim/test/zero", &serde_json::json!(0.0)));
    let val = read_dataref("sim/test/zero").unwrap().as_f64().unwrap();
    assert!(val.abs() < 0.01);
}

#[test]
fn command_empty_name_still_records() {
    clear_mocks();
    assert!(execute_command(""));
    assert_eq!(executed_commands(), vec![""]);
}

#[test]
fn dataref_string_with_unicode() {
    clear_mocks();
    set_mock_dataref(
        "sim/test/unicode",
        MockValue::Data("Ünîcödé✈\0".as_bytes().to_vec()),
    );
    let s = read_dataref_string("sim/test/unicode").unwrap();
    assert_eq!(s, "Ünîcödé✈");
}

// ── Property-based tests ───────────────────────────────────────────────────

proptest! {
    #[test]
    fn prop_ping_preserves_id_and_timestamp(id in 0u32..u32::MAX, ts in 0u64..u64::MAX) {
        let msg = PluginMessage::Ping { id, timestamp: ts };
        if let Some(PluginResponse::Pong { id: rid, timestamp: rts }) = handle_message(msg) {
            prop_assert_eq!(rid, id);
            prop_assert_eq!(rts, ts);
        } else {
            prop_assert!(false, "Ping must return Pong");
        }
    }

    #[test]
    fn prop_get_dataref_missing_returns_error(id in 0u32..u32::MAX, name in "[a-z/]{1,50}") {
        clear_mocks();
        let msg = PluginMessage::GetDataRef { id, name: name.clone() };
        match handle_message(msg) {
            Some(PluginResponse::Error { id: eid, error, .. }) => {
                prop_assert_eq!(eid, Some(id));
                prop_assert!(error.contains("not found"), "Error should say 'not found'");
            }
            other => prop_assert!(false, "Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn prop_float_dataref_round_trip(val in proptest::num::f32::NORMAL) {
        clear_mocks();
        set_mock_dataref("sim/prop/f", MockValue::Float(val));
        let result = read_dataref("sim/prop/f").unwrap();
        let read_val = result.as_f64().unwrap() as f32;
        prop_assert!((read_val - val).abs() < 1e-6 || (read_val == val),
            "Float round-trip mismatch: wrote {val}, read {read_val}");
    }

    #[test]
    fn prop_int_dataref_round_trip(val in proptest::num::i32::ANY) {
        clear_mocks();
        set_mock_dataref("sim/prop/i", MockValue::Int(val));
        let result = read_dataref("sim/prop/i").unwrap();
        prop_assert_eq!(result.as_i64().unwrap() as i32, val);
    }

    #[test]
    fn prop_double_dataref_round_trip(val in proptest::num::f64::NORMAL) {
        clear_mocks();
        set_mock_dataref("sim/prop/d", MockValue::Double(val));
        let result = read_dataref("sim/prop/d").unwrap();
        let read_val = result.as_f64().unwrap();
        prop_assert!((read_val - val).abs() < 1e-10 || (read_val == val),
            "Double round-trip mismatch: wrote {val}, read {read_val}");
    }

    #[test]
    fn prop_command_records_name(name in "[a-z_/]{1,100}") {
        clear_mocks();
        execute_command(&name);
        let cmds = executed_commands();
        prop_assert_eq!(cmds.len(), 1);
        prop_assert_eq!(&cmds[0], &name);
    }

    #[test]
    fn prop_plugin_message_round_trips(id in 0u32..u32::MAX) {
        let msg = PluginMessage::GetAircraftInfo { id };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: PluginMessage = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&decoded).unwrap();
        prop_assert_eq!(json, json2);
    }

    #[test]
    fn prop_pong_response_round_trips(id in 0u32..u32::MAX, ts in 0u64..u64::MAX) {
        let resp = PluginResponse::Pong { id, timestamp: ts };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: PluginResponse = serde_json::from_str(&json).unwrap();
        if let PluginResponse::Pong { id: rid, timestamp: rts } = decoded {
            prop_assert_eq!(rid, id);
            prop_assert_eq!(rts, ts);
        } else {
            prop_assert!(false, "Wrong variant after round-trip");
        }
    }

    #[test]
    fn prop_dataref_string_preserves_ascii(s in "[A-Za-z0-9]{1,20}") {
        clear_mocks();
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0); // NUL terminator
        set_mock_dataref("sim/prop/str", MockValue::Data(bytes));
        let result = read_dataref_string("sim/prop/str").unwrap();
        prop_assert_eq!(result, s);
    }
}
