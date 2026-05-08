// SPDX-License-Identifier: MIT OR Apache-2.0

//! Review telemetry helper.
//!
//! Native Rust replacement for the ad-hoc Python helper previously checked in
//! under `.runs/`.

use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
struct TelemetrySummary {
    files: usize,
}

struct FileDelta {
    file: String,
    delta: u64,
}

/// Generate review telemetry from a git numstat file.
pub fn run_review_telemetry(numstat: &str, json_out: &str, markdown_out: &str) -> Result<()> {
    let numstat = std::fs::read_to_string(numstat)
        .with_context(|| format!("failed to read numstat input {numstat}"))?;
    let mut files = parse_numstat(&numstat);
    files.sort_by(|a, b| b.delta.cmp(&a.delta).then_with(|| a.file.cmp(&b.file)));

    let summary = TelemetrySummary { files: files.len() };
    std::fs::write(
        json_out,
        serde_json::to_string_pretty(&summary).context("failed to serialize telemetry JSON")?,
    )
    .with_context(|| format!("failed to write {}", Path::new(json_out).display()))?;

    let mut lines = vec!["### Start review here".to_owned()];
    lines.extend(
        files
            .iter()
            .take(8)
            .map(|entry| format!("- {}", entry.file)),
    );
    std::fs::write(markdown_out, lines.join("\n"))
        .with_context(|| format!("failed to write {}", Path::new(markdown_out).display()))?;
    Ok(())
}

fn parse_numstat(input: &str) -> Vec<FileDelta> {
    input
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let added = parts.next()?;
            let removed = parts.next()?;
            let file = parts.next()?;
            if added == "-" {
                return None;
            }
            Some(FileDelta {
                file: file.to_owned(),
                delta: added.parse::<u64>().ok()? + removed.parse::<u64>().ok()?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_skips_binary_numstat_lines() {
        let parsed = parse_numstat("2\t3\ta.rs\n-\t-\timage.png\n10\t1\tb.rs\n");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].file, "a.rs");
        assert_eq!(parsed[0].delta, 5);
    }
}
