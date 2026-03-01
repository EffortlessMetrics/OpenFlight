#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz SimVar registry lookup and event catalog with arbitrary strings.
    if let Ok(s) = std::str::from_utf8(data) {
        let registry = flight_simconnect::var_registry::SimVarRegistry::new();
        let _ = registry.get(s);
        let _ = registry.contains(s);

        let _ = flight_simconnect::event_mapping::catalog_lookup(s);
    }
});
