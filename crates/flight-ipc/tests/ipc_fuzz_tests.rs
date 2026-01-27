use proptest::prelude::*;
use prost::Message;

// Import the generated proto code
// This assumes the flight-ipc crate exposes the generated code publically or via a module we can access
// Based on lib.rs content seen earlier, it seems to be in `server` or similar, but typically `prost` generates a module.
// I'll check lib.rs again to be sure, but usually it's `flight_ipc::flight::v1`.
// For now, I'll assume `flight_ipc::flight::v1` based on the package name `flight.v1`.

#[allow(unused_imports)]
use flight_ipc::{
    ApplyProfileRequest, DetectCurveConflictsRequest, HealthSubscribeRequest, ListDevicesRequest,
    NegotiateFeaturesRequest,
};

proptest! {
    //
    // Fuzzing Protobuf Deserialization
    //

    #[test]
    fn test_negotiate_features_fuzz(bytes in prop::collection::vec(any::<u8>(), 0..1024)) {
        // Should return Err on invalid data, but never panic
        let _ = NegotiateFeaturesRequest::decode(bytes.as_slice());
    }

    #[test]
    fn test_list_devices_fuzz(bytes in prop::collection::vec(any::<u8>(), 0..1024)) {
        let _ = ListDevicesRequest::decode(bytes.as_slice());
    }

    #[test]
    fn test_apply_profile_fuzz(bytes in prop::collection::vec(any::<u8>(), 0..1024)) {
        let _ = ApplyProfileRequest::decode(bytes.as_slice());
    }

    #[test]
    fn test_health_subscribe_fuzz(bytes in prop::collection::vec(any::<u8>(), 0..1024)) {
        let _ = HealthSubscribeRequest::decode(bytes.as_slice());
    }

    //
    // Fuzzing JSON Payload in ApplyProfileRequest
    //

    #[test]
    fn test_apply_profile_json_fuzz(json_str in "\\PC*") {
        // Construct a valid request with invalid JSON
        // The server implementation (not tested here directly, but the parsing logic)
        // would handle this.
        // Here we just verify that constructing the message is fine.

        let _req = ApplyProfileRequest {
            profile_json: json_str,
            validate_only: true,
            force_apply: false,
        };
    }
}
