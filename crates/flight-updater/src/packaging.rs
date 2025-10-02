//! Packaging system for MSI (Windows) and systemd user units (Linux)

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;