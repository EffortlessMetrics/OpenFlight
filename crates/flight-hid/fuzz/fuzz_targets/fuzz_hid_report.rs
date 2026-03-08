#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz the HID-support usage extractor with arbitrary descriptor bytes.
    // Must not panic on arbitrary input.
    let _ = flight_hid_support::hid_descriptor::extract_usages(data);
});
