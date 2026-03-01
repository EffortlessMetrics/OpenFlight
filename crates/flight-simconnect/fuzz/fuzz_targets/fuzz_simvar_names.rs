#![no_main]
use libfuzzer_sys::fuzz_target;
use std::sync::LazyLock;

static REGISTRY: LazyLock<flight_simconnect::var_registry::SimVarRegistry> =
    LazyLock::new(flight_simconnect::var_registry::SimVarRegistry::new);

fuzz_target!(|data: &[u8]| {
    // Fuzz SimVar registry lookup and event catalog with arbitrary strings.
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = REGISTRY.get(s);
        let _ = REGISTRY.contains(s);

        let _ = flight_simconnect::event_mapping::catalog_lookup(s);
    }
});
