#![cfg(windows)]
//! Depth tests for flight-simconnect-sys FFI bindings.
//!
//! These tests verify type layouts, enum discriminants, constant values,
//! struct field offsets, trait implementations, and error formatting
//! without requiring a live SimConnect connection.

use std::ffi::c_char;
use std::mem;

use flight_simconnect_sys::constants;
use flight_simconnect_sys::{
    SIMCONNECT_CLIENT_DATA_PERIOD, SIMCONNECT_DATATYPE, SIMCONNECT_EXCEPTION, SIMCONNECT_PERIOD,
    SIMCONNECT_RECV, SIMCONNECT_RECV_EVENT, SIMCONNECT_RECV_EXCEPTION, SIMCONNECT_RECV_ID,
    SIMCONNECT_RECV_OPEN, SIMCONNECT_RECV_SIMOBJECT_DATA, SimConnectError,
};

// ── Type alias sizes ──────────────────────────────────────────────────

#[test]
fn type_alias_hsimconnect_is_pointer_sized() {
    assert_eq!(
        mem::size_of::<flight_simconnect_sys::HSIMCONNECT>(),
        mem::size_of::<*mut ()>(),
        "HSIMCONNECT must be pointer-sized"
    );
}

#[test]
fn type_alias_datadefid_is_u32() {
    assert_eq!(
        mem::size_of::<flight_simconnect_sys::SIMCONNECT_DATADEFID>(),
        mem::size_of::<u32>()
    );
}

#[test]
fn type_alias_requestid_is_u32() {
    assert_eq!(
        mem::size_of::<flight_simconnect_sys::SIMCONNECT_REQUESTID>(),
        mem::size_of::<u32>()
    );
}

#[test]
fn type_alias_eventid_is_u32() {
    assert_eq!(
        mem::size_of::<flight_simconnect_sys::SIMCONNECT_EVENTID>(),
        mem::size_of::<u32>()
    );
}

#[test]
fn type_alias_clienteventid_is_u32() {
    assert_eq!(
        mem::size_of::<flight_simconnect_sys::SIMCONNECT_CLIENTEVENTID>(),
        mem::size_of::<u32>()
    );
}

#[test]
fn type_alias_objectid_is_u32() {
    assert_eq!(
        mem::size_of::<flight_simconnect_sys::SIMCONNECT_OBJECTID>(),
        mem::size_of::<u32>()
    );
}

#[test]
fn type_alias_notificationgroupid_is_u32() {
    assert_eq!(
        mem::size_of::<flight_simconnect_sys::SIMCONNECT_NOTIFICATIONGROUPID>(),
        mem::size_of::<u32>()
    );
}

#[test]
fn type_alias_inputgroupid_is_u32() {
    assert_eq!(
        mem::size_of::<flight_simconnect_sys::SIMCONNECT_INPUTGROUPID>(),
        mem::size_of::<u32>()
    );
}

// ── Struct layout tests ───────────────────────────────────────────────

#[test]
fn struct_recv_layout() {
    // 3 × u32 = 12 bytes, align 4
    assert_eq!(mem::size_of::<SIMCONNECT_RECV>(), 12);
    assert_eq!(mem::align_of::<SIMCONNECT_RECV>(), 4);
}

#[test]
fn struct_recv_exception_layout() {
    // header (12) + 3 × u32 (12) = 24 bytes
    assert_eq!(mem::size_of::<SIMCONNECT_RECV_EXCEPTION>(), 24);
    assert_eq!(mem::align_of::<SIMCONNECT_RECV_EXCEPTION>(), 4);
}

#[test]
fn struct_recv_open_layout() {
    // header (12) + [c_char;256] (256) + 10 × u32 (40) = 308 bytes
    assert_eq!(mem::size_of::<SIMCONNECT_RECV_OPEN>(), 308);
    assert_eq!(mem::align_of::<SIMCONNECT_RECV_OPEN>(), 4);
}

#[test]
fn struct_recv_event_layout() {
    // header (12) + 3 × u32 (12) = 24 bytes
    assert_eq!(mem::size_of::<SIMCONNECT_RECV_EVENT>(), 24);
    assert_eq!(mem::align_of::<SIMCONNECT_RECV_EVENT>(), 4);
}

#[test]
fn struct_recv_simobject_data_layout() {
    // header (12) + 7 × u32 (28) = 40 bytes
    assert_eq!(mem::size_of::<SIMCONNECT_RECV_SIMOBJECT_DATA>(), 40);
    assert_eq!(mem::align_of::<SIMCONNECT_RECV_SIMOBJECT_DATA>(), 4);
}

// ── Struct field offset tests ─────────────────────────────────────────

#[test]
fn struct_recv_field_offsets() {
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV, dwSize), 0);
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV, dwVersion), 4);
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV, dwID), 8);
}

#[test]
fn struct_recv_exception_header_at_offset_zero() {
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV_EXCEPTION, hdr), 0);
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV_EXCEPTION, dwException), 12);
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV_EXCEPTION, dwSendID), 16);
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV_EXCEPTION, dwIndex), 20);
}

#[test]
fn struct_recv_open_field_offsets() {
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV_OPEN, hdr), 0);
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV_OPEN, szApplicationName), 12);
    // After the 256-byte name buffer: 12 + 256 = 268
    assert_eq!(
        mem::offset_of!(SIMCONNECT_RECV_OPEN, dwApplicationVersionMajor),
        268
    );
    assert_eq!(
        mem::offset_of!(SIMCONNECT_RECV_OPEN, dwApplicationVersionMinor),
        272
    );
}

#[test]
fn struct_recv_event_field_offsets() {
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV_EVENT, hdr), 0);
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV_EVENT, uGroupID), 12);
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV_EVENT, uEventID), 16);
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV_EVENT, dwData), 20);
}

#[test]
fn struct_recv_simobject_data_field_offsets() {
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV_SIMOBJECT_DATA, hdr), 0);
    assert_eq!(
        mem::offset_of!(SIMCONNECT_RECV_SIMOBJECT_DATA, dwRequestID),
        12
    );
    assert_eq!(
        mem::offset_of!(SIMCONNECT_RECV_SIMOBJECT_DATA, dwObjectID),
        16
    );
    assert_eq!(
        mem::offset_of!(SIMCONNECT_RECV_SIMOBJECT_DATA, dwDefineID),
        20
    );
    assert_eq!(
        mem::offset_of!(SIMCONNECT_RECV_SIMOBJECT_DATA, dwFlags),
        24
    );
    assert_eq!(
        mem::offset_of!(SIMCONNECT_RECV_SIMOBJECT_DATA, dwDefineCount),
        36
    );
}

// ── Struct zero-initialization ────────────────────────────────────────

#[test]
fn struct_recv_zeroed_is_valid() {
    let recv: SIMCONNECT_RECV = unsafe { mem::zeroed() };
    assert_eq!(recv.dwSize, 0);
    assert_eq!(recv.dwVersion, 0);
    assert_eq!(recv.dwID, 0);
}

#[test]
fn struct_recv_open_name_buffer_is_256() {
    let open: SIMCONNECT_RECV_OPEN = unsafe { mem::zeroed() };
    assert_eq!(open.szApplicationName.len(), 256);
    // All bytes should be zero
    assert!(open.szApplicationName.iter().all(|&b| b == 0));
}

// ── Enum repr(C) size ─────────────────────────────────────────────────

#[test]
fn enum_datatype_is_c_int_sized() {
    // #[repr(C)] enums have the size of a C int (4 bytes on Windows)
    assert_eq!(mem::size_of::<SIMCONNECT_DATATYPE>(), 4);
}

#[test]
fn enum_period_is_c_int_sized() {
    assert_eq!(mem::size_of::<SIMCONNECT_PERIOD>(), 4);
}

#[test]
fn enum_client_data_period_is_c_int_sized() {
    assert_eq!(mem::size_of::<SIMCONNECT_CLIENT_DATA_PERIOD>(), 4);
}

#[test]
fn enum_exception_is_c_int_sized() {
    assert_eq!(mem::size_of::<SIMCONNECT_EXCEPTION>(), 4);
}

#[test]
fn enum_recv_id_is_c_int_sized() {
    assert_eq!(mem::size_of::<SIMCONNECT_RECV_ID>(), 4);
}

// ── Enum discriminant tests ───────────────────────────────────────────

#[test]
fn enum_datatype_discriminants() {
    assert_eq!(SIMCONNECT_DATATYPE::INVALID as u32, 0);
    assert_eq!(SIMCONNECT_DATATYPE::INT32 as u32, 1);
    assert_eq!(SIMCONNECT_DATATYPE::INT64 as u32, 2);
    assert_eq!(SIMCONNECT_DATATYPE::FLOAT32 as u32, 3);
    assert_eq!(SIMCONNECT_DATATYPE::FLOAT64 as u32, 4);
    assert_eq!(SIMCONNECT_DATATYPE::STRING8 as u32, 5);
    assert_eq!(SIMCONNECT_DATATYPE::STRING32 as u32, 6);
    assert_eq!(SIMCONNECT_DATATYPE::STRING64 as u32, 7);
    assert_eq!(SIMCONNECT_DATATYPE::STRING128 as u32, 8);
    assert_eq!(SIMCONNECT_DATATYPE::STRING256 as u32, 9);
    assert_eq!(SIMCONNECT_DATATYPE::STRING260 as u32, 10);
    assert_eq!(SIMCONNECT_DATATYPE::STRINGV as u32, 11);
    assert_eq!(SIMCONNECT_DATATYPE::INITPOSITION as u32, 12);
    assert_eq!(SIMCONNECT_DATATYPE::MARKERSTATE as u32, 13);
    assert_eq!(SIMCONNECT_DATATYPE::WAYPOINT as u32, 14);
    assert_eq!(SIMCONNECT_DATATYPE::LATLONALT as u32, 15);
    assert_eq!(SIMCONNECT_DATATYPE::XYZ as u32, 16);
}

#[test]
fn enum_period_discriminants() {
    assert_eq!(SIMCONNECT_PERIOD::NEVER as u32, 0);
    assert_eq!(SIMCONNECT_PERIOD::ONCE as u32, 1);
    assert_eq!(SIMCONNECT_PERIOD::VISUAL_FRAME as u32, 2);
    assert_eq!(SIMCONNECT_PERIOD::SIM_FRAME as u32, 3);
    assert_eq!(SIMCONNECT_PERIOD::SECOND as u32, 4);
}

#[test]
fn enum_client_data_period_discriminants() {
    assert_eq!(SIMCONNECT_CLIENT_DATA_PERIOD::NEVER as u32, 0);
    assert_eq!(SIMCONNECT_CLIENT_DATA_PERIOD::ONCE as u32, 1);
    assert_eq!(SIMCONNECT_CLIENT_DATA_PERIOD::VISUAL_FRAME as u32, 2);
    assert_eq!(SIMCONNECT_CLIENT_DATA_PERIOD::ON_SET as u32, 3);
}

#[test]
fn enum_exception_discriminants_boundary() {
    assert_eq!(SIMCONNECT_EXCEPTION::NONE as u32, 0);
    assert_eq!(SIMCONNECT_EXCEPTION::ERROR as u32, 1);
    assert_eq!(SIMCONNECT_EXCEPTION::SIZE_MISMATCH as u32, 2);
    assert_eq!(SIMCONNECT_EXCEPTION::UNOPENED as u32, 4);
    assert_eq!(SIMCONNECT_EXCEPTION::INVALID_DATA_TYPE as u32, 18);
    assert_eq!(SIMCONNECT_EXCEPTION::ILLEGAL_OPERATION as u32, 25);
    assert_eq!(SIMCONNECT_EXCEPTION::OBJECT_SCHEDULE as u32, 37);
}

#[test]
fn enum_recv_id_discriminants_boundary() {
    assert_eq!(SIMCONNECT_RECV_ID::NULL as u32, 0);
    assert_eq!(SIMCONNECT_RECV_ID::EXCEPTION as u32, 1);
    assert_eq!(SIMCONNECT_RECV_ID::OPEN as u32, 2);
    assert_eq!(SIMCONNECT_RECV_ID::QUIT as u32, 3);
    assert_eq!(SIMCONNECT_RECV_ID::EVENT as u32, 4);
    assert_eq!(SIMCONNECT_RECV_ID::SIMOBJECT_DATA as u32, 8);
    assert_eq!(SIMCONNECT_RECV_ID::CLIENT_DATA as u32, 16);
    assert_eq!(SIMCONNECT_RECV_ID::EVENT_RACE_LAP as u32, 26);
}

// ── Enum trait derivation tests ───────────────────────────────────────

#[test]
fn enum_datatype_derives_debug_clone_copy_eq() {
    let a = SIMCONNECT_DATATYPE::FLOAT64;
    let b = a; // Copy
    let c = a.clone(); // Clone
    assert_eq!(a, b); // PartialEq
    assert_eq!(b, c); // Eq (via PartialEq)
    let _debug = format!("{:?}", a); // Debug
    assert!(_debug.contains("FLOAT64"));
}

#[test]
fn enum_period_derives_debug_clone_copy_eq() {
    let a = SIMCONNECT_PERIOD::SIM_FRAME;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(a, SIMCONNECT_PERIOD::NEVER);
    assert!(format!("{:?}", a).contains("SIM_FRAME"));
}

#[test]
fn enum_exception_derives_debug_clone_copy_eq() {
    let a = SIMCONNECT_EXCEPTION::ERROR;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(a, SIMCONNECT_EXCEPTION::NONE);
    assert!(format!("{:?}", a).contains("ERROR"));
}

#[test]
fn enum_recv_id_derives_debug_clone_copy_eq() {
    let a = SIMCONNECT_RECV_ID::SIMOBJECT_DATA;
    let b = a;
    assert_eq!(a, b);
    assert!(format!("{:?}", a).contains("SIMOBJECT_DATA"));
}

// ── Struct trait derivation tests ─────────────────────────────────────

#[test]
fn struct_recv_derives_debug_clone_copy() {
    let a = SIMCONNECT_RECV {
        dwSize: 12,
        dwVersion: 1,
        dwID: 0,
    };
    let b = a; // Copy
    let c = a.clone(); // Clone
    assert_eq!(a.dwSize, b.dwSize);
    assert_eq!(b.dwVersion, c.dwVersion);
    let debug = format!("{:?}", a); // Debug
    assert!(debug.contains("dwSize"));
}

#[test]
fn struct_recv_exception_derives_debug_clone_copy() {
    let exc: SIMCONNECT_RECV_EXCEPTION = unsafe { mem::zeroed() };
    let copy = exc;
    assert_eq!(copy.dwException, 0);
    let debug = format!("{:?}", exc);
    assert!(debug.contains("dwException"));
}

// ── Constants tests ───────────────────────────────────────────────────

#[test]
fn constant_object_id_user_is_zero() {
    assert_eq!(constants::SIMCONNECT_OBJECT_ID_USER, 0);
}

#[test]
fn constant_data_request_flags_are_distinct_powers_of_two() {
    let changed = constants::SIMCONNECT_DATA_REQUEST_FLAG_CHANGED;
    let tagged = constants::SIMCONNECT_DATA_REQUEST_FLAG_TAGGED;
    assert_eq!(changed, 0x0000_0001);
    assert_eq!(tagged, 0x0000_0002);
    // Flags must be non-overlapping
    assert_eq!(changed & tagged, 0);
}

#[test]
fn constant_data_definition_ids_are_sequential_from_one() {
    assert_eq!(constants::DATA_DEFINITION_AIRCRAFT, 1);
    assert_eq!(constants::DATA_DEFINITION_KINEMATICS, 2);
    assert_eq!(constants::DATA_DEFINITION_ENGINE, 3);
    assert_eq!(constants::DATA_DEFINITION_ENVIRONMENT, 4);
}

#[test]
fn constant_request_ids_are_sequential_from_one() {
    assert_eq!(constants::REQUEST_AIRCRAFT_DATA, 1);
    assert_eq!(constants::REQUEST_KINEMATICS_DATA, 2);
    assert_eq!(constants::REQUEST_ENGINE_DATA, 3);
    assert_eq!(constants::REQUEST_ENVIRONMENT_DATA, 4);
}

#[test]
fn constant_event_ids_are_sequential_from_one() {
    assert_eq!(constants::EVENT_AIRCRAFT_LOADED, 1);
    assert_eq!(constants::EVENT_SIM_START, 2);
    assert_eq!(constants::EVENT_SIM_STOP, 3);
    assert_eq!(constants::EVENT_PAUSE, 4);
}

// ── Error type tests ──────────────────────────────────────────────────

#[test]
fn error_library_not_found_display() {
    let err = SimConnectError::LibraryNotFound;
    assert_eq!(err.to_string(), "SimConnect library not found");
}

#[test]
fn error_function_not_found_display() {
    let err = SimConnectError::FunctionNotFound("SimConnect_Open".to_string());
    let msg = err.to_string();
    assert!(msg.contains("SimConnect_Open"));
    assert!(msg.contains("Function not found"));
}

#[test]
fn error_invalid_parameter_display() {
    let err = SimConnectError::InvalidParameter;
    assert_eq!(err.to_string(), "Invalid parameter");
}

#[test]
fn error_api_error_display_hex_format() {
    let err = SimConnectError::ApiError(0x80004005_u32 as i32);
    let msg = err.to_string();
    // Should display in hex (uppercase) with 0x prefix
    assert!(msg.contains("80004005"), "got: {msg}");
}

#[test]
fn error_debug_impl_exists() {
    let err = SimConnectError::LibraryNotFound;
    let debug = format!("{:?}", err);
    assert!(debug.contains("LibraryNotFound"));
}

// ── c_char sanity ─────────────────────────────────────────────────────

#[test]
fn c_char_is_one_byte() {
    // On Windows c_char is i8; the RECV_OPEN name buffer depends on this
    assert_eq!(mem::size_of::<c_char>(), 1);
}

// ── Struct embedding (header prefixing) ───────────────────────────────

#[test]
fn recv_exception_starts_with_recv_header() {
    // The first field of RECV_EXCEPTION is SIMCONNECT_RECV, so a pointer
    // to RECV_EXCEPTION can be safely cast to *const SIMCONNECT_RECV.
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV_EXCEPTION, hdr), 0);
    assert_eq!(
        mem::size_of::<SIMCONNECT_RECV>(),
        mem::offset_of!(SIMCONNECT_RECV_EXCEPTION, dwException)
    );
}

#[test]
fn recv_open_starts_with_recv_header() {
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV_OPEN, hdr), 0);
    assert_eq!(
        mem::size_of::<SIMCONNECT_RECV>(),
        mem::offset_of!(SIMCONNECT_RECV_OPEN, szApplicationName)
    );
}

#[test]
fn recv_event_starts_with_recv_header() {
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV_EVENT, hdr), 0);
    assert_eq!(
        mem::size_of::<SIMCONNECT_RECV>(),
        mem::offset_of!(SIMCONNECT_RECV_EVENT, uGroupID)
    );
}

#[test]
fn recv_simobject_data_starts_with_recv_header() {
    assert_eq!(mem::offset_of!(SIMCONNECT_RECV_SIMOBJECT_DATA, hdr), 0);
    assert_eq!(
        mem::size_of::<SIMCONNECT_RECV>(),
        mem::offset_of!(SIMCONNECT_RECV_SIMOBJECT_DATA, dwRequestID)
    );
}

// ── Enum round-trip through discriminant ──────────────────────────────

#[test]
fn enum_datatype_round_trip_via_transmute() {
    // Verify repr(C) discriminant can be read as a raw u32 and back
    for (variant, expected) in [
        (SIMCONNECT_DATATYPE::INVALID, 0u32),
        (SIMCONNECT_DATATYPE::FLOAT64, 4),
        (SIMCONNECT_DATATYPE::XYZ, 16),
    ] {
        let raw: u32 = unsafe { mem::transmute(variant) };
        assert_eq!(raw, expected);
        let back: SIMCONNECT_DATATYPE = unsafe { mem::transmute(raw) };
        assert_eq!(back, variant);
    }
}

#[test]
fn enum_period_round_trip_via_transmute() {
    for (variant, expected) in [
        (SIMCONNECT_PERIOD::NEVER, 0u32),
        (SIMCONNECT_PERIOD::SECOND, 4),
    ] {
        let raw: u32 = unsafe { mem::transmute(variant) };
        assert_eq!(raw, expected);
        let back: SIMCONNECT_PERIOD = unsafe { mem::transmute(raw) };
        assert_eq!(back, variant);
    }
}

// ── Feature gating smoke test ─────────────────────────────────────────

#[test]
fn dynamic_feature_is_default() {
    // The crate's default feature is "dynamic"; verify we can reference
    // SimConnectApi (which exists behind cfg(feature = "dynamic")).
    // We don't instantiate because SimConnect.dll may not be present.
    let _size = mem::size_of::<flight_simconnect_sys::SimConnectApi>();
    assert!(_size > 0, "SimConnectApi should be a non-ZST type");
}
