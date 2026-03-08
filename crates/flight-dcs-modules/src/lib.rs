// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! DCS World aircraft module loader for OpenFlight.
//!
//! Drop `.toml` files — one per aircraft — into a modules directory and use
//! [`ModuleLoader`] to read them at startup. Each file describes the axis
//! count, throttle range, stick throw, and any known quirks for that module.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use flight_dcs_modules::ModuleLoader;
//! use std::path::Path;
//!
//! let mut loader = ModuleLoader::new();
//! let count = loader.load_from_dir(Path::new("modules")).unwrap();
//! println!("Loaded {count} DCS modules");
//!
//! if let Some(m) = loader.get("F/A-18C") {
//!     println!("Hornet: {} axes, stick throw {}°", m.axis_count, m.stick_throw);
//! }
//! ```

// Reserved for future real implementation; silences the unused-extern-crate lint.
#[allow(unused_extern_crates)]
extern crate flight_core;

pub mod loader;
pub mod module;

pub use loader::ModuleLoader;
pub use module::DcsModule;
pub use module::{ControlType, DcsControl};

use thiserror::Error;

/// Errors produced by [`ModuleLoader`].
#[derive(Debug, Error)]
pub enum ModuleError {
    /// An I/O error occurred while reading the modules directory or a file.
    #[error("I/O error reading module: {0}")]
    Io(#[from] std::io::Error),
    /// A `.toml` file could not be parsed as a [`DcsModule`].
    #[error("TOML parse error in {file}: {source}")]
    ParseError {
        file: String,
        #[source]
        source: toml::de::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_fa18c(dir: &TempDir) {
        fs::write(
            dir.path().join("fa-18c.toml"),
            r#"
aircraft = "F/A-18C"
axis_count = 6
throttle_range = [0.0, 1.0]
stick_throw = 45.0
quirks = ["twin-throttle", "catapult-bar"]
"#,
        )
        .unwrap();
    }

    fn write_three_modules(dir: &TempDir) {
        write_fa18c(dir);
        fs::write(
            dir.path().join("f-16c.toml"),
            r#"
aircraft = "F-16C"
axis_count = 5
throttle_range = [0.0, 1.0]
stick_throw = 30.0
quirks = ["side-stick", "fbw"]
"#,
        )
        .unwrap();
        fs::write(
            dir.path().join("a-10c.toml"),
            r#"
aircraft = "A-10C"
axis_count = 7
throttle_range = [0.0, 1.0]
stick_throw = 50.0
quirks = ["twin-throttle", "gun-trigger"]
"#,
        )
        .unwrap();
    }

    #[test]
    fn module_loader_starts_empty() {
        let loader = ModuleLoader::new();
        assert_eq!(loader.len(), 0);
        assert!(loader.is_empty());
    }

    #[test]
    fn load_fa18c_toml_module_found() {
        let dir = TempDir::new().unwrap();
        write_fa18c(&dir);
        let mut loader = ModuleLoader::new();
        loader.load_from_dir(dir.path()).unwrap();
        let m = loader.get("F/A-18C").expect("F/A-18C should be loaded");
        assert_eq!(m.aircraft, "F/A-18C");
        assert_eq!(m.axis_count, 6);
    }

    #[test]
    fn get_unknown_aircraft_returns_none() {
        let loader = ModuleLoader::new();
        assert!(loader.get("B-52").is_none());
    }

    #[test]
    fn load_from_dir_returns_count_of_loaded_modules() {
        let dir = TempDir::new().unwrap();
        write_three_modules(&dir);
        let mut loader = ModuleLoader::new();
        let count = loader.load_from_dir(dir.path()).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn malformed_toml_returns_module_error() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("bad.toml"), "this is not valid toml = [[[").unwrap();
        let mut loader = ModuleLoader::new();
        let err = loader.load_from_dir(dir.path()).unwrap_err();
        assert!(matches!(err, ModuleError::ParseError { .. }));
    }

    #[test]
    fn aircraft_names_returns_all_loaded_names() {
        let dir = TempDir::new().unwrap();
        write_three_modules(&dir);
        let mut loader = ModuleLoader::new();
        loader.load_from_dir(dir.path()).unwrap();
        let names = loader.aircraft_names();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"F/A-18C"));
        assert!(names.contains(&"F-16C"));
        assert!(names.contains(&"A-10C"));
    }

    #[test]
    fn module_serde_roundtrip() {
        let original = DcsModule {
            aircraft: "F/A-18C".to_owned(),
            axis_count: 6,
            throttle_range: [0.0, 1.0],
            stick_throw: 45.0,
            quirks: vec!["twin-throttle".to_owned()],
            version: None,
            description: None,
            controls: vec![],
        };
        let serialized = toml::to_string(&original).expect("serialize");
        let restored: DcsModule = toml::from_str(&serialized).expect("deserialize");
        assert_eq!(restored.aircraft, original.aircraft);
        assert_eq!(restored.axis_count, original.axis_count);
        assert!((restored.stick_throw - original.stick_throw).abs() < f32::EPSILON);
        assert_eq!(restored.quirks, original.quirks);
    }

    #[test]
    fn load_three_modules_len_is_three() {
        let dir = TempDir::new().unwrap();
        write_three_modules(&dir);
        let mut loader = ModuleLoader::new();
        loader.load_from_dir(dir.path()).unwrap();
        assert_eq!(loader.len(), 3);
    }
}
