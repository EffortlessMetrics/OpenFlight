use std::fmt;

/// Category of an error code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    Device,
    Sim,
    Profile,
    Service,
    Plugin,
    Network,
    Config,
    Internal,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Device => "Device",
            Self::Sim => "Simulator",
            Self::Profile => "Profile",
            Self::Service => "Service",
            Self::Plugin => "Plugin",
            Self::Network => "Network",
            Self::Config => "Configuration",
            Self::Internal => "Internal",
        };
        f.write_str(s)
    }
}

/// Metadata for a single error code.
#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub code: &'static str,
    pub category: ErrorCategory,
    pub message: &'static str,
    pub description: &'static str,
    pub resolution: &'static str,
}

/// Comprehensive error code reference for OpenFlight.
pub struct ErrorCatalog;

// Macro to keep the table compact.
macro_rules! entry {
    ($code:expr, $cat:ident, $msg:expr, $desc:expr, $res:expr) => {
        ErrorInfo {
            code: $code,
            category: ErrorCategory::$cat,
            message: $msg,
            description: $desc,
            resolution: $res,
        }
    };
}

static CATALOG: &[ErrorInfo] = &[
    // ── Device ───────────────────────────────────────────────────────
    entry!(
        "DEV-001",
        Device,
        "Device not found",
        "The requested HID device could not be located on any USB bus.",
        "Reconnect the device and check USB cables."
    ),
    entry!(
        "DEV-002",
        Device,
        "Device open failed",
        "The operating system refused to open the HID device.",
        "Run as administrator or check udev rules on Linux."
    ),
    entry!(
        "DEV-003",
        Device,
        "Device read timeout",
        "No data received from device within the expected interval.",
        "Check device firmware and USB connection."
    ),
    entry!(
        "DEV-004",
        Device,
        "Device write failed",
        "Failed to send data to the HID device.",
        "Verify device supports output reports and is connected."
    ),
    entry!(
        "DEV-005",
        Device,
        "Calibration data missing",
        "No calibration data found for this device.",
        "Run the calibration wizard for this device."
    ),
    // ── Sim ──────────────────────────────────────────────────────────
    entry!(
        "SIM-001",
        Sim,
        "SimConnect unavailable",
        "The MSFS SimConnect library could not be loaded.",
        "Ensure MSFS is installed and SimConnect SDK is available."
    ),
    entry!(
        "SIM-002",
        Sim,
        "Sim connection lost",
        "The connection to the simulator was unexpectedly closed.",
        "Restart the simulator and reconnect."
    ),
    entry!(
        "SIM-003",
        Sim,
        "Sim variable not found",
        "The requested simulator variable does not exist.",
        "Check variable name against the sim's SDK documentation."
    ),
    entry!(
        "SIM-004",
        Sim,
        "Sim version mismatch",
        "The simulator version is not compatible with this adapter.",
        "Update OpenFlight or check compatibility matrix."
    ),
    // ── Profile ──────────────────────────────────────────────────────
    entry!(
        "PRF-001",
        Profile,
        "Profile parse error",
        "The profile JSON/TOML could not be parsed.",
        "Validate profile syntax with the profile linter."
    ),
    entry!(
        "PRF-002",
        Profile,
        "Profile schema invalid",
        "The profile does not conform to the expected schema.",
        "Compare against the profile JSON schema in docs/reference."
    ),
    entry!(
        "PRF-003",
        Profile,
        "Profile merge conflict",
        "Two profiles define conflicting axis assignments.",
        "Resolve overlapping assignments in the more-specific profile."
    ),
    entry!(
        "PRF-004",
        Profile,
        "Profile not found",
        "The specified profile file does not exist.",
        "Check the profile path and file name."
    ),
    // ── Service ──────────────────────────────────────────────────────
    entry!(
        "SVC-001",
        Service,
        "Service start failed",
        "The flightd daemon could not start.",
        "Check logs for port conflicts or permission issues."
    ),
    entry!(
        "SVC-002",
        Service,
        "Instance lock held",
        "Another flightd instance is already running.",
        "Stop the existing instance before starting a new one."
    ),
    entry!(
        "SVC-003",
        Service,
        "Graceful shutdown timeout",
        "The service did not shut down within the allowed time.",
        "Force-kill the process and check for stuck tasks."
    ),
    entry!(
        "SVC-004",
        Service,
        "Health check failed",
        "One or more subsystem health checks returned unhealthy.",
        "Run diagnostic bundle and inspect component status."
    ),
    // ── Plugin ───────────────────────────────────────────────────────
    entry!(
        "PLG-001",
        Plugin,
        "Plugin load failed",
        "The plugin could not be loaded from disk.",
        "Verify plugin file exists and is a valid WASM/native module."
    ),
    entry!(
        "PLG-002",
        Plugin,
        "Plugin capability denied",
        "The plugin requested a capability it is not allowed.",
        "Review plugin manifest and grant required capabilities."
    ),
    entry!(
        "PLG-003",
        Plugin,
        "Plugin timeout",
        "The plugin exceeded its per-tick time budget.",
        "Optimise the plugin or increase the budget in config."
    ),
    entry!(
        "PLG-004",
        Plugin,
        "Plugin panic",
        "The plugin panicked during execution.",
        "Check plugin logs and report the bug to the plugin author."
    ),
    // ── Network ──────────────────────────────────────────────────────
    entry!(
        "NET-001",
        Network,
        "gRPC bind failed",
        "The IPC server could not bind to the requested port.",
        "Check if the port is already in use."
    ),
    entry!(
        "NET-002",
        Network,
        "gRPC connection refused",
        "Could not connect to the gRPC endpoint.",
        "Ensure flightd is running and the port is correct."
    ),
    entry!(
        "NET-003",
        Network,
        "TLS handshake failed",
        "The TLS handshake with the remote endpoint failed.",
        "Verify certificates and TLS configuration."
    ),
    entry!(
        "NET-004",
        Network,
        "Cloud sync failed",
        "Profile cloud synchronisation failed.",
        "Check internet connectivity and authentication."
    ),
    // ── Config ───────────────────────────────────────────────────────
    entry!(
        "CFG-001",
        Config,
        "Config file missing",
        "The configuration file was not found at the expected path.",
        "Create a default config file or specify the path explicitly."
    ),
    entry!(
        "CFG-002",
        Config,
        "Config parse error",
        "The configuration file contains invalid syntax.",
        "Validate the file with a TOML/JSON linter."
    ),
    entry!(
        "CFG-003",
        Config,
        "Config value out of range",
        "A configuration value is outside its acceptable range.",
        "Consult docs/reference for valid ranges."
    ),
    entry!(
        "CFG-004",
        Config,
        "Config migration required",
        "The config file uses a deprecated schema version.",
        "Run `flightctl config migrate` to update."
    ),
    // ── Internal ─────────────────────────────────────────────────────
    entry!(
        "INT-001",
        Internal,
        "RT spine overrun",
        "The RT tick exceeded its time budget.",
        "Check system load and disable non-essential plugins."
    ),
    entry!(
        "INT-002",
        Internal,
        "Bus queue full",
        "A lock-free queue dropped messages due to overflow.",
        "Reduce producer rate or increase queue capacity in config."
    ),
    entry!(
        "INT-003",
        Internal,
        "Unexpected state",
        "An internal state machine reached an unexpected state.",
        "File a bug report with diagnostic bundle attached."
    ),
    entry!(
        "INT-004",
        Internal,
        "Allocation in RT path",
        "A heap allocation was detected on the RT hot path.",
        "This is a bug — report it with a backtrace."
    ),
];

impl ErrorCatalog {
    /// Look up an error by its code (e.g. `"DEV-001"`).
    #[must_use]
    pub fn lookup(code: &str) -> Option<&'static ErrorInfo> {
        CATALOG.iter().find(|e| e.code == code)
    }

    /// Return all errors belonging to `category`.
    #[must_use]
    pub fn by_category(category: ErrorCategory) -> Vec<&'static ErrorInfo> {
        CATALOG.iter().filter(|e| e.category == category).collect()
    }

    /// Return every error in the catalog.
    #[must_use]
    pub fn all() -> &'static [ErrorInfo] {
        CATALOG
    }

    /// Format an error code for display.
    #[must_use]
    pub fn format_error(code: &str) -> String {
        match Self::lookup(code) {
            Some(info) => format!(
                "[{}] {} — {}\nResolution: {}",
                info.code, info.message, info.description, info.resolution
            ),
            None => format!("[{code}] Unknown error code"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_at_least_30_entries() {
        assert!(ErrorCatalog::all().len() >= 30);
    }

    #[test]
    fn lookup_existing_code() {
        let info = ErrorCatalog::lookup("DEV-001").unwrap();
        assert_eq!(info.category, ErrorCategory::Device);
    }

    #[test]
    fn lookup_missing_code_returns_none() {
        assert!(ErrorCatalog::lookup("ZZZ-999").is_none());
    }

    #[test]
    fn by_category_device() {
        let devs = ErrorCatalog::by_category(ErrorCategory::Device);
        assert!(devs.len() >= 4);
        assert!(devs.iter().all(|e| e.category == ErrorCategory::Device));
    }

    #[test]
    fn by_category_sim() {
        let sims = ErrorCatalog::by_category(ErrorCategory::Sim);
        assert!(sims.len() >= 4);
    }

    #[test]
    fn all_categories_represented() {
        let cats: std::collections::HashSet<_> =
            ErrorCatalog::all().iter().map(|e| e.category).collect();
        assert!(cats.contains(&ErrorCategory::Device));
        assert!(cats.contains(&ErrorCategory::Sim));
        assert!(cats.contains(&ErrorCategory::Profile));
        assert!(cats.contains(&ErrorCategory::Service));
        assert!(cats.contains(&ErrorCategory::Plugin));
        assert!(cats.contains(&ErrorCategory::Network));
        assert!(cats.contains(&ErrorCategory::Config));
        assert!(cats.contains(&ErrorCategory::Internal));
    }

    #[test]
    fn format_known_error() {
        let s = ErrorCatalog::format_error("SVC-001");
        assert!(s.contains("Service start failed"));
        assert!(s.contains("Resolution:"));
    }

    #[test]
    fn format_unknown_error() {
        let s = ErrorCatalog::format_error("ZZZ-999");
        assert!(s.contains("Unknown error code"));
    }

    #[test]
    fn codes_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for info in ErrorCatalog::all() {
            assert!(seen.insert(info.code), "Duplicate code: {}", info.code);
        }
    }

    #[test]
    fn category_display() {
        assert_eq!(ErrorCategory::Device.to_string(), "Device");
        assert_eq!(ErrorCategory::Config.to_string(), "Configuration");
    }

    // ── Property-based tests ──────────────────────────────────────────────

    use proptest::prelude::*;

    proptest! {
        /// Error codes are unique (verified via random sampling of the catalog).
        #[test]
        fn prop_error_codes_unique(idx in 0usize..100) {
            let all = ErrorCatalog::all();
            if idx < all.len() {
                let code = all[idx].code;
                let count = all.iter().filter(|e| e.code == code).count();
                prop_assert_eq!(count, 1, "duplicate error code: {}", code);
            }
        }

        /// lookup() never panics for any string input.
        #[test]
        fn prop_lookup_never_panics(code in ".*") {
            let _ = ErrorCatalog::lookup(&code);
        }

        /// format_error() never panics for any string input.
        #[test]
        fn prop_format_error_never_panics(code in ".*") {
            let _ = ErrorCatalog::format_error(&code);
        }

        /// Every error in the catalog can be looked up by its own code.
        #[test]
        fn prop_catalog_self_consistent(idx in 0usize..100) {
            let all = ErrorCatalog::all();
            if idx < all.len() {
                let info = &all[idx];
                let looked_up = ErrorCatalog::lookup(info.code);
                prop_assert!(
                    looked_up.is_some(),
                    "code {} not found via lookup",
                    info.code
                );
                prop_assert_eq!(
                    looked_up.unwrap().code, info.code,
                    "lookup returned wrong code"
                );
            }
        }
    }
}
