// SPDX-License-Identifier: MIT OR Apache-2.0

//! Checks for the workspace Clippy policy ledger and manifest wiring.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use toml::Value;

const ACTIVE_RUST_LINTS: &[&str] = &[
    "unsafe_code",
    "unsafe_op_in_unsafe_fn",
    "unused_must_use",
    "unexpected_cfgs",
    "const_item_interior_mutations",
    "function_casts_as_integer",
];

const ACTIVE_CLIPPY_LINTS: &[&str] = &[
    "dbg_macro",
    "todo",
    "unimplemented",
    "panic",
    "unreachable",
    "unwrap_used",
    "expect_used",
    "get_unwrap",
    "unwrap_in_result",
    "panic_in_result_fn",
    "string_slice",
    "indexing_slicing",
    "out_of_bounds_indexing",
    "unchecked_time_subtraction",
    "char_indices_as_byte_indices",
    "sliced_string_as_bytes",
    "index_refutable_slice",
    "let_underscore_future",
    "let_underscore_must_use",
    "let_underscore_lock",
    "unused_result_ok",
    "map_err_ignore",
    "assertions_on_result_states",
    "lines_filter_map_ok",
    "await_holding_lock",
    "await_holding_refcell_ref",
    "await_holding_invalid_type",
    "future_not_send",
    "large_futures",
    "arc_with_non_send_sync",
    "rc_mutex",
    "mut_mutex_lock",
    "readonly_write_lock",
    "mem_forget",
    "forget_non_drop",
    "drop_non_drop",
    "undocumented_unsafe_blocks",
    "multiple_unsafe_ops_per_block",
    "repr_packed_without_abi",
    "float_cmp",
    "float_cmp_const",
    "float_equality_without_abs",
    "lossy_float_literal",
    "cast_sign_loss",
    "cast_possible_wrap",
    "cast_possible_truncation",
    "cast_precision_loss",
    "invalid_upcast_comparisons",
    "cast_abs_to_unsigned",
    "cast_enum_truncation",
    "cast_nan_to_int",
    "manual_midpoint",
    "manual_is_multiple_of",
    "manual_div_ceil",
    "arithmetic_side_effects",
    "suspicious_open_options",
    "nonsensical_open_options",
    "ineffective_open_options",
    "path_buf_push_overwrite",
    "join_absolute_paths",
    "read_line_without_trim",
    "exit",
    "iter_not_returning_iterator",
    "expl_impl_clone_on_copy",
    "infallible_try_from",
    "fallible_impl_from",
    "error_impl_error",
    "result_unit_err",
    "result_large_err",
    "format_in_format_args",
    "to_string_in_format_args",
    "unused_format_specs",
    "unnecessary_debug_formatting",
    "uninlined_format_args",
    "manual_let_else",
    "manual_ok_or",
    "manual_strip",
    "manual_split_once",
    "manual_is_variant_and",
    "filter_map_next",
    "flat_map_option",
    "match_result_ok",
    "cloned_instead_of_copied",
    "iter_cloned_collect",
    "iter_overeager_cloned",
    "needless_collect",
    "redundant_closure",
    "redundant_closure_for_method_calls",
    "missing_panics_doc",
    "missing_errors_doc",
    "allow_attributes",
    "allow_attributes_without_reason",
    "blanket_clippy_restriction_lints",
    "ignore_without_reason",
    "should_panic_without_expect",
];

const FORBIDDEN_TEST_CARVEOUTS: &[&str] = &[
    "allow-unwrap-in-tests",
    "allow-expect-in-tests",
    "allow-panic-in-tests",
    "allow-indexing-slicing-in-tests",
    "allow-dbg-in-tests",
];

#[derive(Debug, Deserialize)]
struct ClippyPolicy {
    schema: u64,
    msrv: String,
    policy: PolicySettings,
    #[serde(default)]
    planned: Vec<PlannedLint>,
}

#[derive(Debug, Deserialize)]
struct PolicySettings {
    panic_free_tests: bool,
    allow_test_carveouts: bool,
    suppression_style: String,
    blanket_categories: bool,
}

#[derive(Debug, Deserialize)]
struct PlannedLint {
    name: String,
    level: String,
    activate_when_msrv: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct ClippyDebt {
    schema: u64,
    #[serde(default)]
    debt: Vec<DebtEntry>,
}

#[derive(Debug, Deserialize)]
struct DebtEntry {
    lint: String,
    path: String,
    owner: String,
    reason: String,
    expires: String,
}

/// Validate lint policy manifests, inheritance, and upgrade-ledger consistency.
///
/// # Errors
///
/// Returns an error if the root manifest, policy ledger, clippy config, or
/// member manifest inheritance does not match the strict workspace policy.
pub fn run_check_lint_policy() -> Result<()> {
    let root_manifest = read_toml(Path::new("Cargo.toml"))?;
    let policy = read_policy()?;
    let debt = read_debt()?;

    let mut errors = Vec::new();

    validate_msrv(&root_manifest, &policy, &mut errors);
    validate_policy_settings(&policy, &mut errors);
    validate_active_lints(&root_manifest, &mut errors);
    validate_planned_lints(&root_manifest, &policy, &mut errors);
    validate_workspace_members(&root_manifest, &mut errors);
    validate_clippy_toml(&mut errors);
    validate_debt(&debt, &mut errors);

    if errors.is_empty() {
        println!("✅ lint policy is coherent");
        println!("   active rust lints: {}", ACTIVE_RUST_LINTS.len());
        println!("   active clippy lints: {}", ACTIVE_CLIPPY_LINTS.len());
        println!("   planned upgrade lints: {}", policy.planned.len());
        println!("   tracked debt entries: {}", debt.debt.len());
        Ok(())
    } else {
        for error in &errors {
            eprintln!("❌ {error}");
        }
        bail!("lint policy check failed with {} issue(s)", errors.len());
    }
}

fn read_policy() -> Result<ClippyPolicy> {
    let text = fs::read_to_string("policy/clippy-lints.toml")
        .context("failed to read policy/clippy-lints.toml")?;
    toml::from_str(&text).context("failed to parse policy/clippy-lints.toml")
}

fn read_debt() -> Result<ClippyDebt> {
    let text = fs::read_to_string("policy/clippy-debt.toml")
        .context("failed to read policy/clippy-debt.toml")?;
    toml::from_str(&text).context("failed to parse policy/clippy-debt.toml")
}

fn read_toml(path: &Path) -> Result<Value> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn validate_msrv(root_manifest: &Value, policy: &ClippyPolicy, errors: &mut Vec<String>) {
    let manifest_msrv = root_manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("rust-version"))
        .and_then(Value::as_str);

    if manifest_msrv != Some(policy.msrv.as_str()) {
        errors.push(format!(
            "workspace.package.rust-version ({:?}) must match policy/clippy-lints.toml msrv ({})",
            manifest_msrv, policy.msrv
        ));
    }
}

fn validate_policy_settings(policy: &ClippyPolicy, errors: &mut Vec<String>) {
    if policy.schema != 1 {
        errors.push(format!(
            "policy/clippy-lints.toml schema must be 1, got {}",
            policy.schema
        ));
    }
    if !policy.policy.panic_free_tests {
        errors.push("policy.panic_free_tests must be true".to_owned());
    }
    if policy.policy.allow_test_carveouts {
        errors.push("policy.allow_test_carveouts must be false".to_owned());
    }
    if policy.policy.suppression_style != "expect-with-reason" {
        errors.push("policy.suppression_style must be expect-with-reason".to_owned());
    }
    if policy.policy.blanket_categories {
        errors.push("policy.blanket_categories must be false".to_owned());
    }
}

fn validate_active_lints(root_manifest: &Value, errors: &mut Vec<String>) {
    let Some(rust_lints) = root_manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("lints"))
        .and_then(|lints| lints.get("rust"))
        .and_then(Value::as_table)
    else {
        errors.push("missing [workspace.lints.rust]".to_owned());
        return;
    };

    let Some(clippy_lints) = root_manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("lints"))
        .and_then(|lints| lints.get("clippy"))
        .and_then(Value::as_table)
    else {
        errors.push("missing [workspace.lints.clippy]".to_owned());
        return;
    };

    for lint in ACTIVE_RUST_LINTS {
        if !rust_lints.contains_key(*lint) {
            errors.push(format!(
                "missing active Rust lint workspace.lints.rust.{lint}"
            ));
        }
    }
    for lint in ACTIVE_CLIPPY_LINTS {
        if !clippy_lints.contains_key(*lint) {
            errors.push(format!(
                "missing active Clippy lint workspace.lints.clippy.{lint}"
            ));
        }
    }
}

fn validate_planned_lints(root_manifest: &Value, policy: &ClippyPolicy, errors: &mut Vec<String>) {
    let clippy_lints = root_manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("lints"))
        .and_then(|lints| lints.get("clippy"))
        .and_then(Value::as_table);

    let planned_names: BTreeSet<&str> = policy
        .planned
        .iter()
        .map(|lint| lint.name.as_str())
        .collect();
    for expected in [
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
    ] {
        if !planned_names.contains(expected) {
            errors.push(format!("missing planned lint {expected}"));
        }
    }

    for planned in &policy.planned {
        if planned.level != "deny" && planned.level != "warn" {
            errors.push(format!(
                "planned lint {} has invalid level {}",
                planned.name, planned.level
            ));
        }
        if planned.reason.trim().is_empty() {
            errors.push(format!("planned lint {} must have a reason", planned.name));
        }
        let lint_key = planned
            .name
            .strip_prefix("clippy::")
            .unwrap_or(planned.name.as_str());
        if version_less_than(&policy.msrv, &planned.activate_when_msrv)
            && clippy_lints.is_some_and(|lints| lints.contains_key(lint_key))
        {
            errors.push(format!(
                "planned lint {} must not be active before MSRV {}",
                planned.name, planned.activate_when_msrv
            ));
        }
    }
}

fn validate_workspace_members(root_manifest: &Value, errors: &mut Vec<String>) {
    let Some(members) = root_manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(Value::as_array)
    else {
        errors.push("workspace.members must be an array".to_owned());
        return;
    };

    for member in members.iter().filter_map(Value::as_str) {
        let manifest = PathBuf::from(member).join("Cargo.toml");
        if !manifest.exists() {
            errors.push(format!("workspace member {} has no Cargo.toml", member));
            continue;
        }
        match read_toml(&manifest) {
            Ok(value) => {
                let inherits = value
                    .get("lints")
                    .and_then(|lints| lints.get("workspace"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if !inherits {
                    errors.push(format!(
                        "{} must contain [lints] workspace = true",
                        manifest.display()
                    ));
                }
            }
            Err(err) => errors.push(err.to_string()),
        }
    }
}

fn validate_clippy_toml(errors: &mut Vec<String>) {
    let text = match fs::read_to_string("clippy.toml") {
        Ok(text) => text,
        Err(err) => {
            errors.push(format!("failed to read clippy.toml: {err}"));
            return;
        }
    };
    for carveout in FORBIDDEN_TEST_CARVEOUTS {
        if text
            .lines()
            .any(|line| line.trim() == format!("{carveout} = true"))
        {
            errors.push(format!(
                "clippy.toml must not enable test carveout {carveout}"
            ));
        }
    }
}

fn validate_debt(debt: &ClippyDebt, errors: &mut Vec<String>) {
    if debt.schema != 1 {
        errors.push(format!(
            "policy/clippy-debt.toml schema must be 1, got {}",
            debt.schema
        ));
    }
    for (idx, entry) in debt.debt.iter().enumerate() {
        let number = idx + 1;
        if entry.lint.trim().is_empty() {
            errors.push(format!("debt entry {number} must include lint"));
        }
        if entry.path.trim().is_empty() {
            errors.push(format!("debt entry {number} must include path"));
        }
        if entry.owner.trim().is_empty() {
            errors.push(format!("debt entry {number} must include owner"));
        }
        if entry.reason.trim().is_empty() {
            errors.push(format!("debt entry {number} must include reason"));
        }
        if entry.expires.trim().is_empty() {
            errors.push(format!("debt entry {number} must include expires"));
        }
    }
}

fn version_less_than(left: &str, right: &str) -> bool {
    parse_version(left) < parse_version(right)
}

fn parse_version(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}
