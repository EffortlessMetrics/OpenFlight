// SPDX-License-Identifier: MIT OR Apache-2.0

//! Lint-policy validation for the governed Clippy baseline.

use anyhow::{Context, Result, bail};
use std::{collections::BTreeMap, fs, path::Path};
use toml::Value;

const ROOT_MANIFEST: &str = "Cargo.toml";
const CLIPPY_LEDGER: &str = "policy/clippy-lints.toml";
const CLIPPY_DEBT: &str = "policy/clippy-debt.toml";
const CLIPPY_CONFIG: &str = "clippy.toml";
const NO_PANIC_ALLOWLIST: &str = "policy/no-panic-allowlist.toml";
const NON_RUST_ALLOWLIST: &str = "policy/non-rust-allowlist.toml";

const TEST_CARVEOUTS: &[&str] = &[
    "allow-unwrap-in-tests",
    "allow-expect-in-tests",
    "allow-panic-in-tests",
    "allow-indexing-slicing-in-tests",
    "allow-dbg-in-tests",
];

const PLANNED_BEFORE_MSRV: &[&str] = &[
    "clippy::same_length_and_capacity",
    "clippy::manual_ilog2",
    "clippy::decimal_bitwise_operands",
    "clippy::needless_type_cast",
    "clippy::disallowed_fields",
    "clippy::manual_checked_ops",
    "clippy::manual_take",
    "clippy::manual_pop_if",
    "clippy::duration_suboptimal_units",
    "clippy::unnecessary_trailing_comma",
];

/// Validate the active lint policy and related ledgers.
pub fn run_check_lint_policy() -> Result<()> {
    let report = evaluate()?;
    if report.errors.is_empty() {
        println!(
            "✅ lint policy OK: {} active lint(s), {} planned lint(s), {} workspace member(s)",
            report.active_lints, report.planned_lints, report.workspace_members
        );
        Ok(())
    } else {
        for error in &report.errors {
            eprintln!("❌ {error}");
        }
        bail!(
            "lint policy check failed with {} error(s)",
            report.errors.len()
        );
    }
}

/// Print a compact policy report and fail if any policy check fails.
pub fn run_policy_report() -> Result<()> {
    let report = evaluate()?;
    println!("lint policy report");
    println!("  workspace members: {}", report.workspace_members);
    println!("  active lints: {}", report.active_lints);
    println!("  planned lints: {}", report.planned_lints);
    println!("  debt entries: {}", report.debt_entries);
    println!("  panic allowlist entries: {}", report.panic_allow_entries);
    println!(
        "  non-rust allowlist entries: {}",
        report.non_rust_allow_entries
    );

    if report.errors.is_empty() {
        println!("  status: ok");
        Ok(())
    } else {
        println!("  status: failed");
        for error in &report.errors {
            eprintln!("❌ {error}");
        }
        bail!("policy report found {} error(s)", report.errors.len());
    }
}

/// Validate the structured no-panic allowlist schema.
pub fn run_check_no_panic_family() -> Result<()> {
    let mut errors = Vec::new();
    let count = validate_allowlist(NO_PANIC_ALLOWLIST, &mut errors)?;
    finish_named_check("no-panic allowlist", count, errors)
}

/// Validate the structured non-Rust file policy allowlist schema.
pub fn run_check_file_policy() -> Result<()> {
    let mut errors = Vec::new();
    let count = validate_allowlist(NON_RUST_ALLOWLIST, &mut errors)?;
    finish_named_check("non-rust file policy", count, errors)
}

fn finish_named_check(name: &str, count: usize, errors: Vec<String>) -> Result<()> {
    if errors.is_empty() {
        println!("✅ {name} OK: {count} allowlist entrie(s)");
        Ok(())
    } else {
        for error in &errors {
            eprintln!("❌ {error}");
        }
        bail!("{name} failed with {} error(s)", errors.len());
    }
}

#[derive(Default)]
struct PolicyReport {
    active_lints: usize,
    planned_lints: usize,
    workspace_members: usize,
    debt_entries: usize,
    panic_allow_entries: usize,
    non_rust_allow_entries: usize,
    errors: Vec<String>,
}

fn evaluate() -> Result<PolicyReport> {
    let root = read_toml(ROOT_MANIFEST)?;
    let ledger = read_toml(CLIPPY_LEDGER)?;
    let debt = read_toml(CLIPPY_DEBT)?;
    let clippy_config = fs::read_to_string(CLIPPY_CONFIG)
        .with_context(|| format!("failed to read {CLIPPY_CONFIG}"))?;

    let mut report = PolicyReport::default();

    let workspace_package_msrv = root
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("rust-version"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let policy_msrv = ledger
        .get("msrv")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if workspace_package_msrv != policy_msrv {
        report.errors.push(format!(
            "workspace.package.rust-version ({workspace_package_msrv}) must match {CLIPPY_LEDGER} msrv ({policy_msrv})"
        ));
    }

    let manifest_lints = root_lint_map(&root, &mut report.errors);
    let ledger_active = ledger_active_lints(&ledger, &mut report.errors);
    report.active_lints = ledger_active.len();
    report.planned_lints = ledger_planned_lints(&ledger, &mut report.errors).len();

    for (name, level) in &manifest_lints {
        match ledger_active.get(name) {
            Some(active_level) if active_level == level => {}
            Some(active_level) => report.errors.push(format!(
                "{CLIPPY_LEDGER} active lint {name} has level {active_level}, expected {level} from root Cargo.toml"
            )),
            None => report.errors.push(format!(
                "root Cargo.toml active lint {name} is missing from {CLIPPY_LEDGER}"
            )),
        }
    }
    for name in ledger_active.keys() {
        if !manifest_lints.contains_key(name) {
            report.errors.push(format!(
                "{CLIPPY_LEDGER} active lint {name} is not active in root Cargo.toml"
            ));
        }
    }

    for planned in PLANNED_BEFORE_MSRV {
        if manifest_lints.contains_key(*planned) {
            report.errors.push(format!(
                "planned upgrade lint {planned} must not be active before its ledger activation MSRV"
            ));
        }
    }

    for carveout in TEST_CARVEOUTS {
        if clippy_config.contains(carveout) && clippy_config.contains(&format!("{carveout} = true"))
        {
            report.errors.push(format!(
                "{CLIPPY_CONFIG} must not enable test carveout {carveout}"
            ));
        }
    }

    report.workspace_members = validate_workspace_lint_inheritance(&root, &mut report.errors)?;
    report.debt_entries = validate_debt(&debt, &mut report.errors);
    report.panic_allow_entries = validate_allowlist(NO_PANIC_ALLOWLIST, &mut report.errors)?;
    report.non_rust_allow_entries = validate_allowlist(NON_RUST_ALLOWLIST, &mut report.errors)?;

    Ok(report)
}

fn read_toml(path: &str) -> Result<Value> {
    let text = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    text.parse::<Value>()
        .with_context(|| format!("failed to parse {path} as TOML"))
}

fn root_lint_map(root: &Value, errors: &mut Vec<String>) -> BTreeMap<String, String> {
    let mut lints = BTreeMap::new();
    let Some(workspace_lints) = root.get("workspace").and_then(|v| v.get("lints")) else {
        errors.push("root Cargo.toml is missing [workspace.lints]".to_string());
        return lints;
    };

    for (section, prefix) in [("rust", ""), ("clippy", "clippy::")] {
        let Some(table) = workspace_lints.get(section).and_then(Value::as_table) else {
            errors.push(format!(
                "root Cargo.toml is missing [workspace.lints.{section}]"
            ));
            continue;
        };
        for (name, level) in table {
            let Some(level) = level.as_str() else {
                errors.push(format!(
                    "workspace lint {section}.{name} must use a string level"
                ));
                continue;
            };
            lints.insert(format!("{prefix}{name}"), level.to_string());
        }
    }

    lints
}

fn ledger_active_lints(ledger: &Value, errors: &mut Vec<String>) -> BTreeMap<String, String> {
    let mut active = BTreeMap::new();
    let Some(entries) = ledger.get("lint").and_then(Value::as_array) else {
        errors.push(format!("{CLIPPY_LEDGER} must contain [[lint]] entries"));
        return active;
    };

    for (index, entry) in entries.iter().enumerate() {
        let status = entry
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if status != "active" {
            continue;
        }
        let Some(name) = required_str(entry, "name", "lint", index, errors) else {
            continue;
        };
        let Some(level) = required_str(entry, "level", "lint", index, errors) else {
            continue;
        };
        for field in ["class", "reason"] {
            let _ = required_str(entry, field, "lint", index, errors);
        }
        active.insert(name.to_string(), level.to_string());
    }

    active
}

fn ledger_planned_lints(ledger: &Value, errors: &mut Vec<String>) -> BTreeMap<String, String> {
    let mut planned = BTreeMap::new();
    let Some(entries) = ledger.get("planned").and_then(Value::as_array) else {
        errors.push(format!("{CLIPPY_LEDGER} must contain [[planned]] entries"));
        return planned;
    };

    for (index, entry) in entries.iter().enumerate() {
        let Some(name) = required_str(entry, "name", "planned", index, errors) else {
            continue;
        };
        let Some(level) = required_str(entry, "level", "planned", index, errors) else {
            continue;
        };
        for field in ["activate_when_msrv", "reason"] {
            let _ = required_str(entry, field, "planned", index, errors);
        }
        planned.insert(name.to_string(), level.to_string());
    }

    planned
}

fn validate_workspace_lint_inheritance(root: &Value, errors: &mut Vec<String>) -> Result<usize> {
    let members = root
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(Value::as_array)
        .context("root Cargo.toml must contain workspace.members")?;

    let mut count = 0;
    for member in members {
        let Some(member) = member.as_str() else {
            errors.push("workspace member entry must be a string".to_string());
            continue;
        };
        count += 1;
        let manifest_path = Path::new(member).join("Cargo.toml");
        let manifest = match read_toml_path(&manifest_path) {
            Ok(value) => value,
            Err(error) => {
                errors.push(error.to_string());
                continue;
            }
        };
        let inherits = manifest
            .get("lints")
            .and_then(|lints| lints.get("workspace"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !inherits {
            errors.push(format!(
                "{} must contain [lints] workspace = true",
                manifest_path.display()
            ));
        }
    }

    Ok(count)
}

fn read_toml_path(path: &Path) -> Result<Value> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    text.parse::<Value>()
        .with_context(|| format!("failed to parse {} as TOML", path.display()))
}

fn validate_debt(debt: &Value, errors: &mut Vec<String>) -> usize {
    let Some(schema) = debt.get("schema").and_then(Value::as_integer) else {
        errors.push(format!("{CLIPPY_DEBT} must declare schema = 1"));
        return 0;
    };
    if schema != 1 {
        errors.push(format!("{CLIPPY_DEBT} schema must be 1"));
    }

    let Some(entries) = debt.get("debt").and_then(Value::as_array) else {
        return 0;
    };
    for (index, entry) in entries.iter().enumerate() {
        for field in ["lint", "path", "owner", "reason", "expires"] {
            let _ = required_str(entry, field, "debt", index, errors);
        }
    }
    entries.len()
}

fn validate_allowlist(path: &str, errors: &mut Vec<String>) -> Result<usize> {
    let value = read_toml(path)?;
    let Some(_schema) = value.get("schema_version").or_else(|| value.get("schema")) else {
        errors.push(format!("{path} must declare a schema version"));
        return Ok(0);
    };

    let Some(entries) = value.get("allow").and_then(Value::as_array) else {
        return Ok(0);
    };

    for (index, entry) in entries.iter().enumerate() {
        let has_path = entry.get("path").and_then(Value::as_str).is_some();
        let has_glob = entry.get("glob").and_then(Value::as_str).is_some();
        if !has_path && !has_glob {
            errors.push(format!("{path} allow[{index}] must contain path or glob"));
        }
        for field in ["owner", "classification"] {
            let _ = required_str(entry, field, "allow", index, errors);
        }
        if path == NO_PANIC_ALLOWLIST {
            for field in ["family", "explanation"] {
                let _ = required_str(entry, field, "allow", index, errors);
            }
            if entry.get("selector").and_then(Value::as_table).is_none() {
                errors.push(format!(
                    "{path} allow[{index}] must contain [allow.selector]"
                ));
            }
        } else if path == NON_RUST_ALLOWLIST {
            for field in ["kind", "reason", "surface"] {
                let _ = required_str(entry, field, "allow", index, errors);
            }
            if entry.get("covered_by").and_then(Value::as_array).is_none() {
                errors.push(format!("{path} allow[{index}] must contain covered_by"));
            }
        }
    }

    Ok(entries.len())
}

fn required_str<'a>(
    table: &'a Value,
    field: &str,
    kind: &str,
    index: usize,
    errors: &mut Vec<String>,
) -> Option<&'a str> {
    let value = table.get(field).and_then(Value::as_str);
    if value.is_none_or(str::is_empty) {
        errors.push(format!("{kind}[{index}] must contain non-empty {field}"));
    }
    value
}
