// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Export.lua bridge for DCS integration.
//!
//! Generates the Export.lua hook snippet that DCS needs to send telemetry
//! to Flight Hub, detects installed snippet versions, and provides safe
//! auto-install/uninstall that preserves other Export.lua consumers
//! (e.g. SRS, TacView, Helios, VAICOM).

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Current snippet version embedded in generated code.
pub const SNIPPET_VERSION: &str = "1.1.0";

/// Marker comments that delimit our snippet inside a shared Export.lua.
pub const SNIPPET_BEGIN_MARKER: &str = "-- [FlightHub:BEGIN]";
pub const SNIPPET_END_MARKER: &str = "-- [FlightHub:END]";

/// Well-known third-party Export.lua consumers.
pub const KNOWN_CONSUMERS: &[&str] = &[
    "SRS",
    "Tacview",
    "TacView",
    "Helios",
    "VAICOM",
    "DCS-BIOS",
    "LotAtc",
    "Scratchpad",
];

// ---------------------------------------------------------------------------
// Snippet generation
// ---------------------------------------------------------------------------

/// Configuration for the generated Lua snippet.
#[derive(Debug, Clone)]
pub struct LuaBridgeConfig {
    /// Target host for UDP telemetry.
    pub host: String,
    /// Target port for UDP telemetry.
    pub port: u16,
    /// Update rate in Hz.
    pub rate_hz: f32,
}

impl Default for LuaBridgeConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 7778,
            rate_hz: 60.0,
        }
    }
}

/// Generate the Flight Hub Export.lua hook snippet.
///
/// The snippet is delimited by begin/end markers so it can be safely
/// appended to or removed from an existing Export.lua file that already
/// contains other tools' hooks.
pub fn generate_snippet(config: &LuaBridgeConfig) -> String {
    let interval = if config.rate_hz > 0.0 {
        1.0 / config.rate_hz as f64
    } else {
        1.0 / 60.0
    };

    format!(
        r#"{begin}
-- Flight Hub DCS Export snippet v{version}
-- Auto-generated — do not edit between the markers.

do
  local FH = {{}}
  FH.host = "{host}"
  FH.port = {port}
  FH.interval = {interval:.6}
  FH.lastT = 0

  local socket = require("socket")
  FH.udp = socket.udp()
  FH.udp:settimeout(0)
  FH.udp:setpeername(FH.host, FH.port)

  -- Chain previous hooks
  local _start  = LuaExportStart
  local _stop   = LuaExportStop
  local _before = LuaExportBeforeNextFrame
  local _after  = LuaExportAfterNextFrame
  local _next   = LuaExportActivityNextEvent

  function LuaExportStart()
    if _start then _start() end
  end

  function LuaExportStop()
    if FH.udp then FH.udp:close() end
    if _stop then _stop() end
  end

  function LuaExportBeforeNextFrame()
    if _before then _before() end
  end

  function LuaExportAfterNextFrame()
    if _after then _after() end
    local t = LoGetModelTime and LoGetModelTime() or 0
    if t - FH.lastT >= FH.interval then
      FH.lastT = t
      local sd = LoGetSelfData and LoGetSelfData()
      if sd then
        local msg = string.format(
          "HEADER:timestamp=%.3f,model_time=%.3f,aircraft=%s\n",
          t, t, sd.Name or "Unknown")
        FH.udp:send(msg)
      end
    end
  end

  function LuaExportActivityNextEvent(t)
    local prev = _next and _next(t)
    local ours = t + FH.interval
    if prev and prev < ours then return prev end
    return ours
  end
end
{end}"#,
        begin = SNIPPET_BEGIN_MARKER,
        end = SNIPPET_END_MARKER,
        version = SNIPPET_VERSION,
        host = config.host,
        port = config.port,
        interval = interval,
    )
}

// ---------------------------------------------------------------------------
// Version detection
// ---------------------------------------------------------------------------

/// Detected snippet status on disk.
#[derive(Debug, Clone, PartialEq)]
pub enum SnippetStatus {
    /// No Flight Hub snippet found.
    NotInstalled,
    /// Snippet installed with the given version.
    Installed { version: String },
    /// Markers found but version string is missing or unparseable.
    Corrupted,
}

impl fmt::Display for SnippetStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SnippetStatus::NotInstalled => write!(f, "Not installed"),
            SnippetStatus::Installed { version } => write!(f, "Installed v{version}"),
            SnippetStatus::Corrupted => write!(f, "Corrupted"),
        }
    }
}

/// Detect the Flight Hub snippet version from a file's content.
pub fn detect_snippet_version(content: &str) -> SnippetStatus {
    if !content.contains(SNIPPET_BEGIN_MARKER) {
        return SnippetStatus::NotInstalled;
    }
    // Look for `snippet v<VERSION>` between markers
    if let Some(start_idx) = content.find(SNIPPET_BEGIN_MARKER) {
        let after = &content[start_idx..];
        if let Some(ver_start) = after.find("snippet v") {
            let ver_rest = &after[ver_start + "snippet v".len()..];
            let ver_end = ver_rest
                .find(|c: char| !c.is_ascii_digit() && c != '.')
                .unwrap_or(ver_rest.len());
            let version = ver_rest[..ver_end].to_string();
            if !version.is_empty() {
                return SnippetStatus::Installed { version };
            }
        }
    }
    SnippetStatus::Corrupted
}

/// Detect snippet version from an Export.lua file on disk.
pub fn detect_snippet_version_from_file(path: &Path) -> std::io::Result<SnippetStatus> {
    if !path.exists() {
        return Ok(SnippetStatus::NotInstalled);
    }
    let content = fs::read_to_string(path)?;
    Ok(detect_snippet_version(&content))
}

/// Returns `true` if the installed snippet version matches [`SNIPPET_VERSION`].
pub fn is_up_to_date(content: &str) -> bool {
    matches!(
        detect_snippet_version(content),
        SnippetStatus::Installed { version } if version == SNIPPET_VERSION
    )
}

// ---------------------------------------------------------------------------
// Third-party consumer detection
// ---------------------------------------------------------------------------

/// Detect known third-party Export.lua consumers present in the file content.
pub fn detect_consumers(content: &str) -> Vec<&'static str> {
    KNOWN_CONSUMERS
        .iter()
        .filter(|&&name| content.contains(name))
        .copied()
        .collect()
}

// ---------------------------------------------------------------------------
// Install / uninstall
// ---------------------------------------------------------------------------

/// Result of an install or uninstall operation.
#[derive(Debug, Clone, PartialEq)]
pub enum HookAction {
    /// Snippet was freshly installed.
    Installed,
    /// Snippet was updated from an older version.
    Updated { old_version: String },
    /// Snippet was already up to date — no changes.
    AlreadyUpToDate,
    /// Snippet was removed.
    Removed,
    /// File did not contain our snippet — nothing to remove.
    NothingToRemove,
}

impl fmt::Display for HookAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HookAction::Installed => write!(f, "Installed"),
            HookAction::Updated { old_version } => write!(f, "Updated from v{old_version}"),
            HookAction::AlreadyUpToDate => write!(f, "Already up to date"),
            HookAction::Removed => write!(f, "Removed"),
            HookAction::NothingToRemove => write!(f, "Nothing to remove"),
        }
    }
}

/// Install (or update) the Flight Hub snippet into `export_lua_path`.
///
/// If the file does not exist it is created. If it exists, the snippet is
/// appended or the existing Flight Hub block is replaced. Other content in
/// the file (SRS, TacView, etc.) is preserved.
pub fn install_hook(
    export_lua_path: &Path,
    config: &LuaBridgeConfig,
) -> std::io::Result<HookAction> {
    let snippet = generate_snippet(config);

    if !export_lua_path.exists() {
        if let Some(parent) = export_lua_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(export_lua_path, &snippet)?;
        return Ok(HookAction::Installed);
    }

    let existing = fs::read_to_string(export_lua_path)?;
    let status = detect_snippet_version(&existing);

    match status {
        SnippetStatus::NotInstalled => {
            // Append our snippet
            let mut new_content = existing;
            if !new_content.ends_with('\n') {
                new_content.push('\n');
            }
            new_content.push('\n');
            new_content.push_str(&snippet);
            fs::write(export_lua_path, new_content)?;
            Ok(HookAction::Installed)
        }
        SnippetStatus::Installed { ref version } if version == SNIPPET_VERSION => {
            Ok(HookAction::AlreadyUpToDate)
        }
        SnippetStatus::Installed { ref version } => {
            let old_version = version.clone();
            let new_content = replace_snippet_block(&existing, &snippet);
            fs::write(export_lua_path, new_content)?;
            Ok(HookAction::Updated { old_version })
        }
        SnippetStatus::Corrupted => {
            let new_content = replace_snippet_block(&existing, &snippet);
            fs::write(export_lua_path, new_content)?;
            Ok(HookAction::Updated {
                old_version: "unknown".to_string(),
            })
        }
    }
}

/// Remove the Flight Hub snippet from `export_lua_path`, preserving all
/// other content.
pub fn remove_hook(export_lua_path: &Path) -> std::io::Result<HookAction> {
    if !export_lua_path.exists() {
        return Ok(HookAction::NothingToRemove);
    }

    let existing = fs::read_to_string(export_lua_path)?;
    if !existing.contains(SNIPPET_BEGIN_MARKER) {
        return Ok(HookAction::NothingToRemove);
    }

    let new_content = remove_snippet_block(&existing);
    let trimmed = new_content.trim();
    if trimmed.is_empty() {
        fs::remove_file(export_lua_path)?;
    } else {
        fs::write(export_lua_path, new_content)?;
    }
    Ok(HookAction::Removed)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Replace the Flight Hub block (markers inclusive) in `content` with `snippet`.
fn replace_snippet_block(content: &str, snippet: &str) -> String {
    if let (Some(start), Some(end_start)) = (
        content.find(SNIPPET_BEGIN_MARKER),
        content.find(SNIPPET_END_MARKER),
    ) {
        let end = end_start + SNIPPET_END_MARKER.len();
        // Skip trailing newline after end marker
        let end = if content[end..].starts_with('\n') {
            end + 1
        } else if content[end..].starts_with("\r\n") {
            end + 2
        } else {
            end
        };
        let mut out = String::with_capacity(content.len());
        out.push_str(&content[..start]);
        out.push_str(snippet);
        out.push_str(&content[end..]);
        out
    } else {
        // Markers incomplete — append fresh
        let mut out = content.to_string();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(snippet);
        out
    }
}

/// Remove the Flight Hub block from `content`.
fn remove_snippet_block(content: &str) -> String {
    if let (Some(start), Some(end_start)) = (
        content.find(SNIPPET_BEGIN_MARKER),
        content.find(SNIPPET_END_MARKER),
    ) {
        let end = end_start + SNIPPET_END_MARKER.len();
        let end = if content[end..].starts_with('\n') {
            end + 1
        } else if content[end..].starts_with("\r\n") {
            end + 2
        } else {
            end
        };
        let mut out = String::with_capacity(content.len());
        out.push_str(&content[..start]);
        out.push_str(&content[end..]);
        out
    } else {
        content.to_string()
    }
}

/// Resolve the default Export.lua path for a given DCS saved-games directory.
pub fn export_lua_path(dcs_saved_games: &Path) -> PathBuf {
    dcs_saved_games.join("Scripts").join("Export.lua")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn default_config() -> LuaBridgeConfig {
        LuaBridgeConfig::default()
    }

    // --- Snippet generation ---

    #[test]
    fn test_generate_snippet_contains_markers() {
        let snippet = generate_snippet(&default_config());
        assert!(snippet.starts_with(SNIPPET_BEGIN_MARKER));
        assert!(snippet.ends_with(SNIPPET_END_MARKER));
    }

    #[test]
    fn test_generate_snippet_contains_version() {
        let snippet = generate_snippet(&default_config());
        assert!(snippet.contains(&format!("snippet v{}", SNIPPET_VERSION)));
    }

    #[test]
    fn test_generate_snippet_contains_host_port() {
        let config = LuaBridgeConfig {
            host: "10.0.0.5".to_string(),
            port: 9999,
            rate_hz: 30.0,
        };
        let snippet = generate_snippet(&config);
        assert!(snippet.contains("FH.host = \"10.0.0.5\""));
        assert!(snippet.contains("FH.port = 9999"));
    }

    #[test]
    fn test_generate_snippet_chains_hooks() {
        let snippet = generate_snippet(&default_config());
        assert!(snippet.contains("local _start  = LuaExportStart"));
        assert!(snippet.contains("local _stop   = LuaExportStop"));
        assert!(snippet.contains("if _start then _start() end"));
        assert!(snippet.contains("if _stop then _stop() end"));
    }

    #[test]
    fn test_generate_snippet_uses_udp() {
        let snippet = generate_snippet(&default_config());
        assert!(snippet.contains("socket.udp()"));
        assert!(snippet.contains("settimeout(0)"));
    }

    #[test]
    fn test_generate_snippet_interval_calculation() {
        let config = LuaBridgeConfig {
            rate_hz: 30.0,
            ..default_config()
        };
        let snippet = generate_snippet(&config);
        // 1/30 ≈ 0.033333
        assert!(snippet.contains("FH.interval = 0.033333"));
    }

    // --- Version detection ---

    #[test]
    fn test_detect_version_not_installed() {
        assert_eq!(
            detect_snippet_version("-- some other lua code"),
            SnippetStatus::NotInstalled
        );
    }

    #[test]
    fn test_detect_version_installed() {
        let snippet = generate_snippet(&default_config());
        let status = detect_snippet_version(&snippet);
        assert_eq!(
            status,
            SnippetStatus::Installed {
                version: SNIPPET_VERSION.to_string()
            }
        );
    }

    #[test]
    fn test_detect_version_old() {
        let content = format!(
            "{}\n-- Flight Hub DCS Export snippet v0.9.0\nsome code\n{}",
            SNIPPET_BEGIN_MARKER, SNIPPET_END_MARKER
        );
        let status = detect_snippet_version(&content);
        assert_eq!(
            status,
            SnippetStatus::Installed {
                version: "0.9.0".to_string()
            }
        );
    }

    #[test]
    fn test_detect_version_corrupted() {
        let content = format!(
            "{}\n-- no version line here\n{}",
            SNIPPET_BEGIN_MARKER, SNIPPET_END_MARKER
        );
        assert_eq!(detect_snippet_version(&content), SnippetStatus::Corrupted);
    }

    #[test]
    fn test_is_up_to_date() {
        let snippet = generate_snippet(&default_config());
        assert!(is_up_to_date(&snippet));
        assert!(!is_up_to_date("-- empty file"));
    }

    #[test]
    fn test_detect_version_from_missing_file() {
        let status =
            detect_snippet_version_from_file(Path::new("/nonexistent/Export.lua")).unwrap();
        assert_eq!(status, SnippetStatus::NotInstalled);
    }

    // --- Consumer detection ---

    #[test]
    fn test_detect_consumers_srs_tacview() {
        let content = "-- SRS integration\nlocal TacView = require('TacView')";
        let consumers = detect_consumers(content);
        assert!(consumers.contains(&"SRS"));
        assert!(consumers.contains(&"TacView"));
    }

    #[test]
    fn test_detect_consumers_none() {
        let consumers = detect_consumers("-- plain export lua");
        assert!(consumers.is_empty());
    }

    #[test]
    fn test_detect_consumers_helios() {
        let content = "dofile(lfs.writedir()..[[Scripts\\Helios\\HeliosExport.lua]])";
        let consumers = detect_consumers(content);
        assert!(consumers.contains(&"Helios"));
    }

    // --- Install hook ---

    #[test]
    fn test_install_to_new_file() {
        let tmp = TempDir::new().unwrap();
        let path = export_lua_path(tmp.path());
        let action = install_hook(&path, &default_config()).unwrap();
        assert_eq!(action, HookAction::Installed);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains(SNIPPET_BEGIN_MARKER));
        assert!(content.contains(SNIPPET_END_MARKER));
        assert!(is_up_to_date(&content));
    }

    #[test]
    fn test_install_appends_to_existing() {
        let tmp = TempDir::new().unwrap();
        let scripts_dir = tmp.path().join("Scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        let path = scripts_dir.join("Export.lua");

        let existing = "-- SRS hook\nlocal SRS = require('SRS')\n";
        fs::write(&path, existing).unwrap();

        let action = install_hook(&path, &default_config()).unwrap();
        assert_eq!(action, HookAction::Installed);

        let content = fs::read_to_string(&path).unwrap();
        // SRS content preserved
        assert!(content.contains("SRS"));
        // Our snippet appended
        assert!(content.contains(SNIPPET_BEGIN_MARKER));
    }

    #[test]
    fn test_install_already_up_to_date() {
        let tmp = TempDir::new().unwrap();
        let path = export_lua_path(tmp.path());

        install_hook(&path, &default_config()).unwrap();
        let action = install_hook(&path, &default_config()).unwrap();
        assert_eq!(action, HookAction::AlreadyUpToDate);
    }

    #[test]
    fn test_install_updates_old_version() {
        let tmp = TempDir::new().unwrap();
        let scripts_dir = tmp.path().join("Scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        let path = scripts_dir.join("Export.lua");

        let old_snippet = format!(
            "{}\n-- Flight Hub DCS Export snippet v0.5.0\nold code here\n{}\n",
            SNIPPET_BEGIN_MARKER, SNIPPET_END_MARKER
        );
        fs::write(&path, &old_snippet).unwrap();

        let action = install_hook(&path, &default_config()).unwrap();
        assert!(matches!(action, HookAction::Updated { old_version } if old_version == "0.5.0"));

        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("v0.5.0"));
        assert!(content.contains(&format!("v{}", SNIPPET_VERSION)));
    }

    #[test]
    fn test_install_preserves_surrounding_content() {
        let tmp = TempDir::new().unwrap();
        let scripts_dir = tmp.path().join("Scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        let path = scripts_dir.join("Export.lua");

        let content = format!(
            "-- Before\n{}\n-- snippet v0.1.0\nold\n{}\n-- After\n",
            SNIPPET_BEGIN_MARKER, SNIPPET_END_MARKER
        );
        fs::write(&path, &content).unwrap();

        install_hook(&path, &default_config()).unwrap();

        let updated = fs::read_to_string(&path).unwrap();
        assert!(updated.contains("-- Before"));
        assert!(updated.contains("-- After"));
        assert!(updated.contains(&format!("v{}", SNIPPET_VERSION)));
    }

    // --- Remove hook ---

    #[test]
    fn test_remove_hook() {
        let tmp = TempDir::new().unwrap();
        let path = export_lua_path(tmp.path());
        install_hook(&path, &default_config()).unwrap();

        let action = remove_hook(&path).unwrap();
        assert_eq!(action, HookAction::Removed);
        // File should be removed since it was only our snippet
        assert!(!path.exists());
    }

    #[test]
    fn test_remove_hook_preserves_other_content() {
        let tmp = TempDir::new().unwrap();
        let scripts_dir = tmp.path().join("Scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        let path = scripts_dir.join("Export.lua");

        let snippet = generate_snippet(&default_config());
        let content = format!("-- SRS\n{snippet}\n-- TacView\n");
        fs::write(&path, &content).unwrap();

        let action = remove_hook(&path).unwrap();
        assert_eq!(action, HookAction::Removed);

        let remaining = fs::read_to_string(&path).unwrap();
        assert!(remaining.contains("-- SRS"));
        assert!(remaining.contains("-- TacView"));
        assert!(!remaining.contains(SNIPPET_BEGIN_MARKER));
    }

    #[test]
    fn test_remove_hook_nothing_to_remove() {
        let tmp = TempDir::new().unwrap();
        let scripts_dir = tmp.path().join("Scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        let path = scripts_dir.join("Export.lua");
        fs::write(&path, "-- SRS only").unwrap();

        let action = remove_hook(&path).unwrap();
        assert_eq!(action, HookAction::NothingToRemove);
    }

    #[test]
    fn test_remove_hook_file_missing() {
        let action = remove_hook(Path::new("/tmp/nonexistent/Export.lua")).unwrap();
        assert_eq!(action, HookAction::NothingToRemove);
    }

    // --- Snippet content validation ---

    #[test]
    fn test_snippet_contains_loget_self_data() {
        let snippet = generate_snippet(&default_config());
        assert!(snippet.contains("LoGetSelfData"));
    }

    #[test]
    fn test_snippet_contains_activity_next_event() {
        let snippet = generate_snippet(&default_config());
        assert!(snippet.contains("LuaExportActivityNextEvent"));
    }

    #[test]
    fn test_snippet_zero_rate_defaults_60hz() {
        let config = LuaBridgeConfig {
            rate_hz: 0.0,
            ..default_config()
        };
        let snippet = generate_snippet(&config);
        // Should default to 1/60 ≈ 0.016667
        assert!(snippet.contains("0.016667"));
    }

    // --- Display impls ---

    #[test]
    fn test_snippet_status_display() {
        assert_eq!(SnippetStatus::NotInstalled.to_string(), "Not installed");
        assert_eq!(
            SnippetStatus::Installed {
                version: "1.0.0".to_string()
            }
            .to_string(),
            "Installed v1.0.0"
        );
        assert_eq!(SnippetStatus::Corrupted.to_string(), "Corrupted");
    }

    #[test]
    fn test_hook_action_display() {
        assert_eq!(HookAction::Installed.to_string(), "Installed");
        assert_eq!(
            HookAction::Updated {
                old_version: "0.9.0".to_string()
            }
            .to_string(),
            "Updated from v0.9.0"
        );
        assert_eq!(
            HookAction::AlreadyUpToDate.to_string(),
            "Already up to date"
        );
        assert_eq!(HookAction::Removed.to_string(), "Removed");
        assert_eq!(HookAction::NothingToRemove.to_string(), "Nothing to remove");
    }

    // --- Export.lua path ---

    #[test]
    fn test_export_lua_path() {
        let p = export_lua_path(Path::new("/home/user/Saved Games/DCS"));
        assert!(p.ends_with("Export.lua"));
        assert!(p.to_string_lossy().contains("Scripts"));
    }
}
