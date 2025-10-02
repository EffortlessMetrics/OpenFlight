//! Error Taxonomy and Stable Error Codes
//!
//! Provides a comprehensive taxonomy of error codes with stable identifiers
//! linked to knowledge base articles for troubleshooting and support.

use std::fmt;
use serde::{Deserialize, Serialize};

/// Stable error code with knowledge base link
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ErrorCode {
    /// Stable error identifier (e.g., "HID_OUT_STALL")
    pub code: String,
    /// Error category
    pub category: ErrorCategory,
    /// Human-readable description
    pub description: String,
    /// Knowledge base article URL
    pub kb_url: Option<String>,
}

/// Error categories for organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// Hardware and device communication errors
    Hardware,
    /// Real-time performance and timing errors
    Performance,
    /// Safety system and interlock errors
    Safety,
    /// Configuration and profile errors
    Configuration,
    /// Simulator integration errors
    Simulator,
    /// Plugin and extension errors
    Plugin,
    /// System and platform errors
    System,
}

/// Stable error with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StableError {
    /// Error code
    pub code: ErrorCode,
    /// Additional context information
    pub context: std::collections::HashMap<String, String>,
    /// Timestamp when error occurred
    pub timestamp: u64,
    /// Component that generated the error
    pub component: String,
}

/// Error taxonomy manager
pub struct ErrorTaxonomy {
    /// Registry of all known error codes
    error_codes: std::collections::HashMap<String, ErrorCode>,
}

impl ErrorTaxonomy {
    /// Create new error taxonomy with predefined codes
    pub fn new() -> Self {
        let mut taxonomy = Self {
            error_codes: std::collections::HashMap::new(),
        };
        
        taxonomy.register_standard_errors();
        taxonomy
    }
    
    /// Register all standard error codes
    fn register_standard_errors(&mut self) {
        // Hardware/Device Errors
        self.register_error(ErrorCode {
            code: "HID_OUT_STALL".to_string(),
            category: ErrorCategory::Hardware,
            description: "USB HID output endpoint stalled for ≥3 frames".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/HID_OUT_STALL".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "HID_ENDPOINT_WEDGED".to_string(),
            category: ErrorCategory::Hardware,
            description: "USB HID endpoint is wedged and unresponsive".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/HID_ENDPOINT_WEDGED".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "DEVICE_DISCONNECT".to_string(),
            category: ErrorCategory::Hardware,
            description: "Flight control device unexpectedly disconnected".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/DEVICE_DISCONNECT".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "DEVICE_OVERTEMP".to_string(),
            category: ErrorCategory::Hardware,
            description: "Device reported over-temperature condition".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/DEVICE_OVERTEMP".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "DEVICE_OVERCURRENT".to_string(),
            category: ErrorCategory::Hardware,
            description: "Device reported over-current condition".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/DEVICE_OVERCURRENT".to_string()),
        });
        
        // Performance Errors
        self.register_error(ErrorCode {
            code: "AXIS_JITTER".to_string(),
            category: ErrorCategory::Performance,
            description: "Real-time axis loop timing violation (jitter >0.5ms p99)".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/AXIS_JITTER".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "MISSED_TICK".to_string(),
            category: ErrorCategory::Performance,
            description: "Axis processing tick missed deadline (>6ms at 250Hz)".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/MISSED_TICK".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "HID_LATENCY_HIGH".to_string(),
            category: ErrorCategory::Performance,
            description: "HID write latency exceeded threshold (>300μs p99)".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/HID_LATENCY_HIGH".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "ALLOC_IN_RT_PATH".to_string(),
            category: ErrorCategory::Performance,
            description: "Memory allocation detected in real-time path".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/ALLOC_IN_RT_PATH".to_string()),
        });
        
        // Safety Errors
        self.register_error(ErrorCode {
            code: "FFB_FAULT".to_string(),
            category: ErrorCategory::Safety,
            description: "Force feedback safety fault detected".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/FFB_FAULT".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "TORQUE_LIMIT_EXCEEDED".to_string(),
            category: ErrorCategory::Safety,
            description: "Force feedback torque exceeded safety limits".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/TORQUE_LIMIT_EXCEEDED".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "INTERLOCK_FAILURE".to_string(),
            category: ErrorCategory::Safety,
            description: "Safety interlock system failure".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/INTERLOCK_FAILURE".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "WATCHDOG_TIMEOUT".to_string(),
            category: ErrorCategory::Safety,
            description: "Component watchdog timeout detected".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/WATCHDOG_TIMEOUT".to_string()),
        });
        
        // Configuration Errors
        self.register_error(ErrorCode {
            code: "PROFILE_INVALID".to_string(),
            category: ErrorCategory::Configuration,
            description: "Profile validation failed - invalid configuration".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/PROFILE_INVALID".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "CURVE_NON_MONOTONIC".to_string(),
            category: ErrorCategory::Configuration,
            description: "Profile curve is not monotonic".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/CURVE_NON_MONOTONIC".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "WRITER_MISMATCH".to_string(),
            category: ErrorCategory::Configuration,
            description: "Simulator configuration drift detected".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/WRITER_MISMATCH".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "PROFILE_COMPILE_FAILED".to_string(),
            category: ErrorCategory::Configuration,
            description: "Profile compilation to pipeline failed".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/PROFILE_COMPILE_FAILED".to_string()),
        });
        
        // Simulator Errors
        self.register_error(ErrorCode {
            code: "SIM_DISCONNECT".to_string(),
            category: ErrorCategory::Simulator,
            description: "Simulator connection lost".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/SIM_DISCONNECT".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "SIMCONNECT_FAILED".to_string(),
            category: ErrorCategory::Simulator,
            description: "SimConnect initialization or communication failed".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/SIMCONNECT_FAILED".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "XPLANE_DATAREF_FAILED".to_string(),
            category: ErrorCategory::Simulator,
            description: "X-Plane DataRef access failed".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/XPLANE_DATAREF_FAILED".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "DCS_EXPORT_FAILED".to_string(),
            category: ErrorCategory::Simulator,
            description: "DCS Export.lua communication failed".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/DCS_EXPORT_FAILED".to_string()),
        });
        
        // Plugin Errors
        self.register_error(ErrorCode {
            code: "PLUG_OVERRUN".to_string(),
            category: ErrorCategory::Plugin,
            description: "Plugin exceeded time budget and was quarantined".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/PLUG_OVERRUN".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "PLUG_CRASH".to_string(),
            category: ErrorCategory::Plugin,
            description: "Plugin process crashed".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/PLUG_CRASH".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "PLUG_CAPABILITY_DENIED".to_string(),
            category: ErrorCategory::Plugin,
            description: "Plugin attempted to use undeclared capability".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/PLUG_CAPABILITY_DENIED".to_string()),
        });
        
        // System Errors
        self.register_error(ErrorCode {
            code: "RT_PRIVILEGE_DENIED".to_string(),
            category: ErrorCategory::System,
            description: "Real-time privileges not available".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/RT_PRIVILEGE_DENIED".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "POWER_THROTTLING_ACTIVE".to_string(),
            category: ErrorCategory::System,
            description: "System power throttling affecting performance".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/POWER_THROTTLING_ACTIVE".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "MEMORY_EXHAUSTED".to_string(),
            category: ErrorCategory::System,
            description: "System memory exhausted".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/MEMORY_EXHAUSTED".to_string()),
        });
        
        self.register_error(ErrorCode {
            code: "IPC_FAILED".to_string(),
            category: ErrorCategory::System,
            description: "Inter-process communication failed".to_string(),
            kb_url: Some("https://docs.flight-hub.dev/kb/IPC_FAILED".to_string()),
        });
    }
    
    /// Register a new error code
    pub fn register_error(&mut self, error_code: ErrorCode) {
        self.error_codes.insert(error_code.code.clone(), error_code);
    }
    
    /// Get error code by identifier
    pub fn get_error(&self, code: &str) -> Option<&ErrorCode> {
        self.error_codes.get(code)
    }
    
    /// Get all error codes in a category
    pub fn get_errors_by_category(&self, category: ErrorCategory) -> Vec<&ErrorCode> {
        self.error_codes
            .values()
            .filter(|e| e.category == category)
            .collect()
    }
    
    /// Create a stable error with context
    pub fn create_error(
        &self,
        code: &str,
        component: &str,
        context: std::collections::HashMap<String, String>,
    ) -> Option<StableError> {
        self.get_error(code).map(|error_code| {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            
            StableError {
                code: error_code.clone(),
                context,
                timestamp,
                component: component.to_string(),
            }
        })
    }
    
    /// Get all registered error codes
    pub fn get_all_errors(&self) -> Vec<&ErrorCode> {
        self.error_codes.values().collect()
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code)
    }
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorCategory::Hardware => write!(f, "Hardware"),
            ErrorCategory::Performance => write!(f, "Performance"),
            ErrorCategory::Safety => write!(f, "Safety"),
            ErrorCategory::Configuration => write!(f, "Configuration"),
            ErrorCategory::Simulator => write!(f, "Simulator"),
            ErrorCategory::Plugin => write!(f, "Plugin"),
            ErrorCategory::System => write!(f, "System"),
        }
    }
}

impl Default for ErrorTaxonomy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_taxonomy_creation() {
        let taxonomy = ErrorTaxonomy::new();
        
        // Should have standard errors registered
        assert!(taxonomy.get_error("HID_OUT_STALL").is_some());
        assert!(taxonomy.get_error("AXIS_JITTER").is_some());
        assert!(taxonomy.get_error("FFB_FAULT").is_some());
    }
    
    #[test]
    fn test_error_categories() {
        let taxonomy = ErrorTaxonomy::new();
        
        let hardware_errors = taxonomy.get_errors_by_category(ErrorCategory::Hardware);
        assert!(!hardware_errors.is_empty());
        
        let performance_errors = taxonomy.get_errors_by_category(ErrorCategory::Performance);
        assert!(!performance_errors.is_empty());
    }
    
    #[test]
    fn test_stable_error_creation() {
        let taxonomy = ErrorTaxonomy::new();
        let mut context = std::collections::HashMap::new();
        context.insert("device_id".to_string(), "test_device".to_string());
        
        let error = taxonomy.create_error("HID_OUT_STALL", "hid_adapter", context);
        assert!(error.is_some());
        
        let error = error.unwrap();
        assert_eq!(error.code.code, "HID_OUT_STALL");
        assert_eq!(error.component, "hid_adapter");
        assert!(error.context.contains_key("device_id"));
    }
    
    #[test]
    fn test_error_code_display() {
        let error_code = ErrorCode {
            code: "TEST_ERROR".to_string(),
            category: ErrorCategory::System,
            description: "Test error".to_string(),
            kb_url: None,
        };
        
        assert_eq!(error_code.to_string(), "TEST_ERROR");
    }
    
    #[test]
    fn test_error_category_display() {
        assert_eq!(ErrorCategory::Hardware.to_string(), "Hardware");
        assert_eq!(ErrorCategory::Performance.to_string(), "Performance");
        assert_eq!(ErrorCategory::Safety.to_string(), "Safety");
    }
}