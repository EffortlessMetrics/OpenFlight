#![no_main]

use bytemuck::try_from_bytes;
use flight_falcon_bms::FlightData;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(fd) = try_from_bytes::<FlightData>(data) {
        let _ = fd.pitch_normalized();
        let _ = fd.roll_normalized();
        let _ = fd.yaw_normalized();
        let _ = fd.throttle_normalized();
    }
});
