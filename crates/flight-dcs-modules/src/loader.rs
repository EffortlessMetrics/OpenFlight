// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! TOML-based loader for DCS World aircraft module configurations.

use std::collections::HashMap;
use std::path::Path;

use crate::ModuleError;
use crate::module::DcsModule;

/// Loads and caches [`DcsModule`] configurations from TOML files.
///
/// Call [`load_from_dir`](Self::load_from_dir) to populate the loader from a
/// directory of `.toml` files, then retrieve modules by aircraft name via
/// [`get`](Self::get).
pub struct ModuleLoader {
    modules: HashMap<String, DcsModule>,
}

impl ModuleLoader {
    /// Create an empty loader.
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }

    /// Read every `.toml` file in `path`, parse it as a [`DcsModule`], and
    /// store it keyed by [`DcsModule::aircraft`].
    ///
    /// Returns the number of modules successfully loaded.
    ///
    /// # Errors
    ///
    /// - [`ModuleError::Io`] — the directory cannot be read.
    /// - [`ModuleError::ParseError`] — a `.toml` file is malformed.
    pub fn load_from_dir(&mut self, path: &Path) -> Result<usize, ModuleError> {
        let mut count = 0usize;

        let entries = std::fs::read_dir(path)?;
        for entry in entries {
            let entry = entry?;
            let file_path = entry.path();
            if file_path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }

            let src = std::fs::read_to_string(&file_path)?;
            let file_name = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("<unknown>")
                .to_owned();

            let module: DcsModule =
                toml::from_str(&src).map_err(|source| ModuleError::ParseError {
                    file: file_name,
                    source,
                })?;

            tracing::debug!(aircraft = %module.aircraft, "loaded DCS module");
            self.modules.insert(module.aircraft.clone(), module);
            count += 1;
        }

        Ok(count)
    }

    /// Look up a module by its aircraft identifier.
    pub fn get(&self, aircraft: &str) -> Option<&DcsModule> {
        self.modules.get(aircraft)
    }

    /// Number of currently loaded modules.
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Returns `true` when no modules are loaded.
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// Sorted list of loaded aircraft identifiers.
    pub fn aircraft_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.modules.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    /// Return all loaded modules whose [`DcsModule::version`] equals `version`.
    pub fn modules_with_version(&self, version: &str) -> Vec<&DcsModule> {
        self.modules
            .values()
            .filter(|m| m.version.as_deref() == Some(version))
            .collect()
    }

    /// Iterate over all loaded modules (unordered).
    pub fn all_modules(&self) -> impl Iterator<Item = &DcsModule> {
        self.modules.values()
    }
}

impl Default for ModuleLoader {
    fn default() -> Self {
        Self::new()
    }
}
