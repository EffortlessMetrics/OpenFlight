use serde_json::Value;
use std::collections::HashSet;

/// Severity level of a lint diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// An auto-fix suggestion attached to a lint diagnostic.
#[derive(Debug, Clone)]
pub struct FixSuggestion {
    pub description: String,
    pub path: String,
    pub suggested_value: Option<Value>,
}

/// Result of linting a profile configuration.
pub struct LintResult {
    pub warnings: Vec<LintWarning>,
    pub errors: Vec<LintError>,
}

impl LintResult {
    /// Returns all diagnostics that carry a fix suggestion.
    #[must_use]
    pub fn fixes(&self) -> Vec<&FixSuggestion> {
        let mut out: Vec<&FixSuggestion> = Vec::new();
        for w in &self.warnings {
            if let Some(ref f) = w.fix {
                out.push(f);
            }
        }
        for e in &self.errors {
            if let Some(ref f) = e.fix {
                out.push(f);
            }
        }
        out
    }

    /// Returns `true` when no errors or warnings were produced.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.warnings.is_empty() && self.errors.is_empty()
    }

    /// Returns a machine-parseable summary: `code:severity:path:message` lines.
    #[must_use]
    pub fn to_machine_readable(&self) -> String {
        let mut lines = Vec::new();
        for w in &self.warnings {
            lines.push(format!(
                "{}:{}:{}:{}",
                w.code,
                severity_str(w.severity),
                w.path,
                w.message
            ));
        }
        for e in &self.errors {
            lines.push(format!(
                "{}:{}:{}:{}",
                e.code,
                severity_str(e.severity),
                e.path,
                e.message
            ));
        }
        lines.join("\n")
    }
}

fn severity_str(s: Severity) -> &'static str {
    match s {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

/// A non-fatal lint finding.
pub struct LintWarning {
    pub code: String,
    pub message: String,
    pub path: String,
    pub severity: Severity,
    pub fix: Option<FixSuggestion>,
}

/// A fatal lint finding.
pub struct LintError {
    pub code: String,
    pub message: String,
    pub path: String,
    pub severity: Severity,
    pub fix: Option<FixSuggestion>,
}

/// A single lint rule applied to profile JSON.
pub struct LintRule {
    pub code: String,
    pub description: String,
    pub priority: i32,
    checker: CheckerFn,
}

type CheckerFn = Box<dyn Fn(&Value) -> (Vec<LintWarning>, Vec<LintError>) + Send + Sync>;

/// Static analyser for profile configurations.
pub struct ProfileLinter {
    rules: Vec<LintRule>,
    disabled_rules: HashSet<String>,
}

impl ProfileLinter {
    /// Creates a linter pre-loaded with all built-in rules.
    #[must_use]
    pub fn new() -> Self {
        let mut linter = Self {
            rules: Vec::new(),
            disabled_rules: HashSet::new(),
        };
        linter.add_builtin_rules();
        linter
    }

    /// Registers an additional lint rule.
    pub fn add_rule(&mut self, rule: LintRule) {
        self.rules.push(rule);
    }

    /// Disables a rule by its code so it is skipped during linting.
    pub fn disable_rule(&mut self, code: &str) {
        self.disabled_rules.insert(code.to_owned());
    }

    /// Re-enables a previously disabled rule.
    pub fn enable_rule(&mut self, code: &str) {
        self.disabled_rules.remove(code);
    }

    /// Returns the codes of all registered rules in priority order.
    #[must_use]
    pub fn rule_codes(&self) -> Vec<&str> {
        let mut sorted: Vec<&LintRule> = self.rules.iter().collect();
        sorted.sort_by_key(|r| r.priority);
        sorted.iter().map(|r| r.code.as_str()).collect()
    }

    /// Runs every enabled rule against `profile_json` (ordered by priority)
    /// and returns the aggregated warnings and errors.
    pub fn lint(&self, profile_json: &Value) -> LintResult {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        let mut sorted: Vec<&LintRule> = self.rules.iter().collect();
        sorted.sort_by_key(|r| r.priority);
        for rule in sorted {
            if self.disabled_rules.contains(&rule.code) {
                continue;
            }
            let (w, e) = (rule.checker)(profile_json);
            warnings.extend(w);
            errors.extend(e);
        }
        LintResult { warnings, errors }
    }

    /// Applies all fix suggestions to a copy of the profile and returns it.
    #[must_use]
    pub fn auto_fix(&self, profile_json: &Value) -> Value {
        let result = self.lint(profile_json);
        let mut fixed = profile_json.clone();
        for fix in result.fixes() {
            if let Some(ref val) = fix.suggested_value {
                set_path(&mut fixed, &fix.path, val.clone());
            }
        }
        fixed
    }

    fn add_builtin_rules(&mut self) {
        self.rules.push(Self::rule_overlapping_axes());
        self.rules.push(Self::rule_missing_required_fields());
        self.rules.push(Self::rule_deadzone_range());
        self.rules.push(Self::rule_curve_monotonicity());
        self.rules.push(Self::rule_unused_device_refs());
        self.rules.push(Self::rule_expo_bounds());
        self.rules.push(Self::rule_axis_name_validation());
        self.rules.push(Self::rule_schema_version());
        self.rules.push(Self::rule_conflicting_curve_expo());
        self.rules.push(Self::rule_deprecated_fields());
    }

    // ── Built-in rules ───────────────────────────────────────────────

    fn rule_overlapping_axes() -> LintRule {
        LintRule {
            code: "E001".into(),
            description: "Overlapping axis assignments".into(),
            priority: 0,
            checker: Box::new(|profile| {
                let mut errors = Vec::new();
                if let Some(axes) = profile.get("axes").and_then(Value::as_array) {
                    let mut seen = HashSet::new();
                    for (i, axis) in axes.iter().enumerate() {
                        if let Some(id) = axis.get("assignment").and_then(Value::as_str)
                            && !seen.insert(id.to_owned())
                        {
                            errors.push(LintError {
                                code: "E001".into(),
                                message: format!("Duplicate axis assignment '{id}'"),
                                path: format!("axes[{i}].assignment"),
                                severity: Severity::Error,
                                fix: None,
                            });
                        }
                    }
                }
                (Vec::new(), errors)
            }),
        }
    }

    fn rule_missing_required_fields() -> LintRule {
        LintRule {
            code: "E002".into(),
            description: "Missing required fields".into(),
            priority: 0,
            checker: Box::new(|profile| {
                let mut errors = Vec::new();
                let required = ["name", "version"];
                for field in required {
                    if profile.get(field).is_none() {
                        errors.push(LintError {
                            code: "E002".into(),
                            message: format!("Missing required field '{field}'"),
                            path: field.to_owned(),
                            severity: Severity::Error,
                            fix: Some(FixSuggestion {
                                description: format!("Add default value for '{field}'"),
                                path: field.to_owned(),
                                suggested_value: Some(Value::String(
                                    if field == "version" {
                                        "1.0".to_owned()
                                    } else {
                                        "unnamed".to_owned()
                                    },
                                )),
                            }),
                        });
                    }
                }
                (Vec::new(), errors)
            }),
        }
    }

    fn rule_deadzone_range() -> LintRule {
        LintRule {
            code: "W001".into(),
            description: "Deadzone value out of range (0–50%)".into(),
            priority: 10,
            checker: Box::new(|profile| {
                let mut warnings = Vec::new();
                if let Some(axes) = profile.get("axes").and_then(Value::as_array) {
                    for (i, axis) in axes.iter().enumerate() {
                        if let Some(dz) = axis.get("deadzone").and_then(Value::as_f64)
                            && !(0.0..=50.0).contains(&dz)
                        {
                            let clamped = dz.clamp(0.0, 50.0);
                            warnings.push(LintWarning {
                                code: "W001".into(),
                                message: format!("Deadzone {dz}% is outside the valid 0–50% range"),
                                path: format!("axes[{i}].deadzone"),
                                severity: Severity::Warning,
                                fix: Some(FixSuggestion {
                                    description: format!("Clamp deadzone to {clamped}%"),
                                    path: format!("axes[{i}].deadzone"),
                                    suggested_value: Some(Value::from(clamped)),
                                }),
                            });
                        }
                    }
                }
                (warnings, Vec::new())
            }),
        }
    }

    fn rule_curve_monotonicity() -> LintRule {
        LintRule {
            code: "W002".into(),
            description: "Curve coefficients produce non-monotonic output".into(),
            priority: 10,
            checker: Box::new(|profile| {
                let mut warnings = Vec::new();
                if let Some(axes) = profile.get("axes").and_then(Value::as_array) {
                    for (i, axis) in axes.iter().enumerate() {
                        if let Some(coeffs) = axis.get("curve").and_then(Value::as_array) {
                            let c: Vec<f64> = coeffs.iter().filter_map(Value::as_f64).collect();
                            if !c.is_empty() && !is_monotonic(&c) {
                                warnings.push(LintWarning {
                                    code: "W002".into(),
                                    message: "Curve coefficients may produce non-monotonic output"
                                        .into(),
                                    path: format!("axes[{i}].curve"),
                                    severity: Severity::Warning,
                                    fix: None,
                                });
                            }
                        }
                    }
                }
                (warnings, Vec::new())
            }),
        }
    }

    fn rule_unused_device_refs() -> LintRule {
        LintRule {
            code: "W003".into(),
            description: "Unused device references".into(),
            priority: 20,
            checker: Box::new(|profile| {
                let mut warnings = Vec::new();
                let mut declared = HashSet::new();
                let mut referenced = HashSet::new();

                if let Some(devices) = profile.get("devices").and_then(Value::as_array) {
                    for d in devices {
                        if let Some(id) = d.get("id").and_then(Value::as_str) {
                            declared.insert(id.to_owned());
                        }
                    }
                }
                if let Some(axes) = profile.get("axes").and_then(Value::as_array) {
                    for a in axes {
                        if let Some(id) = a.get("device_id").and_then(Value::as_str) {
                            referenced.insert(id.to_owned());
                        }
                    }
                }
                for id in &declared {
                    if !referenced.contains(id) {
                        warnings.push(LintWarning {
                            code: "W003".into(),
                            message: format!("Device '{id}' is declared but never referenced"),
                            path: "devices".into(),
                            severity: Severity::Warning,
                            fix: None,
                        });
                    }
                }
                (warnings, Vec::new())
            }),
        }
    }

    fn rule_expo_bounds() -> LintRule {
        LintRule {
            code: "W004".into(),
            description: "Expo value out of range (-1.0 to 1.0)".into(),
            priority: 10,
            checker: Box::new(|profile| {
                let mut warnings = Vec::new();
                if let Some(axes) = profile.get("axes").and_then(Value::as_array) {
                    for (i, axis) in axes.iter().enumerate() {
                        if let Some(expo) = axis.get("expo").and_then(Value::as_f64)
                            && !(-1.0..=1.0).contains(&expo)
                        {
                            let clamped = expo.clamp(-1.0, 1.0);
                            warnings.push(LintWarning {
                                code: "W004".into(),
                                message: format!(
                                    "Expo {expo} is outside the valid -1.0 to 1.0 range"
                                ),
                                path: format!("axes[{i}].expo"),
                                severity: Severity::Warning,
                                fix: Some(FixSuggestion {
                                    description: format!("Clamp expo to {clamped}"),
                                    path: format!("axes[{i}].expo"),
                                    suggested_value: Some(Value::from(clamped)),
                                }),
                            });
                        }
                    }
                }
                (warnings, Vec::new())
            }),
        }
    }

    fn rule_axis_name_validation() -> LintRule {
        LintRule {
            code: "E003".into(),
            description: "Axis assignment name must be alphanumeric/underscore".into(),
            priority: 0,
            checker: Box::new(|profile| {
                let mut errors = Vec::new();
                if let Some(axes) = profile.get("axes").and_then(Value::as_array) {
                    for (i, axis) in axes.iter().enumerate() {
                        if let Some(name) = axis.get("assignment").and_then(Value::as_str)
                            && (name.is_empty()
                                || !name
                                    .chars()
                                    .all(|c| c.is_ascii_alphanumeric() || c == '_'))
                        {
                            errors.push(LintError {
                                code: "E003".into(),
                                message: format!(
                                    "Invalid axis name '{name}': must be alphanumeric/underscore"
                                ),
                                path: format!("axes[{i}].assignment"),
                                severity: Severity::Error,
                                fix: None,
                            });
                        }
                    }
                }
                (Vec::new(), errors)
            }),
        }
    }

    fn rule_schema_version() -> LintRule {
        LintRule {
            code: "E004".into(),
            description: "Profile schema version must be a valid version string".into(),
            priority: 0,
            checker: Box::new(|profile| {
                let mut errors = Vec::new();
                if let Some(ver) = profile.get("version").and_then(Value::as_str) {
                    let valid = ver.split('.').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()));
                    if !valid || ver.is_empty() {
                        errors.push(LintError {
                            code: "E004".into(),
                            message: format!(
                                "Invalid schema version '{ver}': expected numeric dotted format (e.g. '1.0')"
                            ),
                            path: "version".into(),
                            severity: Severity::Error,
                            fix: None,
                        });
                    }
                }
                (Vec::new(), errors)
            }),
        }
    }

    fn rule_conflicting_curve_expo() -> LintRule {
        LintRule {
            code: "W005".into(),
            description: "Axis has both curve and expo set — curve takes precedence".into(),
            priority: 10,
            checker: Box::new(|profile| {
                let mut warnings = Vec::new();
                if let Some(axes) = profile.get("axes").and_then(Value::as_array) {
                    for (i, axis) in axes.iter().enumerate() {
                        let has_curve = axis.get("curve").is_some();
                        let has_expo = axis.get("expo").is_some();
                        if has_curve && has_expo {
                            warnings.push(LintWarning {
                                code: "W005".into(),
                                message: "Axis has both 'curve' and 'expo' set; curve takes precedence".into(),
                                path: format!("axes[{i}]"),
                                severity: Severity::Warning,
                                fix: None,
                            });
                        }
                    }
                }
                (warnings, Vec::new())
            }),
        }
    }

    fn rule_deprecated_fields() -> LintRule {
        LintRule {
            code: "W006".into(),
            description: "Deprecated field detected".into(),
            priority: 20,
            checker: Box::new(|profile| {
                let mut warnings = Vec::new();
                let deprecated = [("sensitivity", "expo"), ("response_curve", "curve")];
                if let Some(axes) = profile.get("axes").and_then(Value::as_array) {
                    for (i, axis) in axes.iter().enumerate() {
                        for (old, new) in &deprecated {
                            if axis.get(*old).is_some() {
                                warnings.push(LintWarning {
                                    code: "W006".into(),
                                    message: format!(
                                        "Deprecated field '{old}': migrate to '{new}'"
                                    ),
                                    path: format!("axes[{i}].{old}"),
                                    severity: Severity::Info,
                                    fix: Some(FixSuggestion {
                                        description: format!("Rename '{old}' to '{new}'"),
                                        path: format!("axes[{i}].{old}"),
                                        suggested_value: None,
                                    }),
                                });
                            }
                        }
                    }
                }
                (warnings, Vec::new())
            }),
        }
    }
}

impl Default for ProfileLinter {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluate polynomial coefficients at 11 sample points and check monotonicity.
fn is_monotonic(coeffs: &[f64]) -> bool {
    let steps = 10;
    let mut prev = eval_poly(coeffs, 0.0);
    for i in 1..=steps {
        let x = i as f64 / steps as f64;
        let y = eval_poly(coeffs, x);
        if y < prev {
            return false;
        }
        prev = y;
    }
    true
}

fn eval_poly(coeffs: &[f64], x: f64) -> f64 {
    coeffs
        .iter()
        .enumerate()
        .map(|(i, &c)| c * x.powi(i as i32))
        .sum()
}

/// Set a value at a simple JSON path (supports `field` and `axes[0].field`).
fn set_path(root: &mut Value, path: &str, value: Value) {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = root;
    for (idx, part) in parts.iter().enumerate() {
        let is_last = idx == parts.len() - 1;
        // Check for array index: e.g. "axes[0]"
        if let Some(bracket) = part.find('[') {
            let key = &part[..bracket];
            let index_str = &part[bracket + 1..part.len() - 1];
            if let Ok(i) = index_str.parse::<usize>() {
                if is_last {
                    if let Some(arr) = current.get_mut(key).and_then(Value::as_array_mut)
                        && i < arr.len()
                    {
                        arr[i] = value;
                    }
                    return;
                }
                current = &mut current[key][i];
            } else {
                return;
            }
        } else if is_last {
            current[part] = value;
            return;
        } else {
            current = &mut current[*part];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn linter() -> ProfileLinter {
        ProfileLinter::new()
    }

    fn valid_profile() -> Value {
        json!({
            "name": "default",
            "version": "1.0",
            "axes": [
                {"assignment": "roll", "deadzone": 5.0, "device_id": "dev1"},
                {"assignment": "pitch", "deadzone": 3.0, "device_id": "dev1"}
            ],
            "devices": [{"id": "dev1"}]
        })
    }

    // ── Existing built-in rule tests (kept) ──────────────────────────

    #[test]
    fn valid_profile_passes() {
        let r = linter().lint(&valid_profile());
        assert!(r.errors.is_empty());
    }

    #[test]
    fn detects_overlapping_axes() {
        let p = json!({
            "name": "test", "version": "1",
            "axes": [
                {"assignment": "roll"},
                {"assignment": "roll"}
            ]
        });
        let r = linter().lint(&p);
        assert!(r.errors.iter().any(|e| e.code == "E001"));
    }

    #[test]
    fn detects_missing_name() {
        let p = json!({"version": "1"});
        let r = linter().lint(&p);
        assert!(
            r.errors
                .iter()
                .any(|e| e.code == "E002" && e.path == "name")
        );
    }

    #[test]
    fn detects_missing_version() {
        let p = json!({"name": "x"});
        let r = linter().lint(&p);
        assert!(
            r.errors
                .iter()
                .any(|e| e.code == "E002" && e.path == "version")
        );
    }

    #[test]
    fn deadzone_in_range_no_warning() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"deadzone": 25.0}]
        });
        let r = linter().lint(&p);
        assert!(!r.warnings.iter().any(|w| w.code == "W001"));
    }

    #[test]
    fn deadzone_out_of_range() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"deadzone": 75.0}]
        });
        let r = linter().lint(&p);
        assert!(r.warnings.iter().any(|w| w.code == "W001"));
    }

    #[test]
    fn non_monotonic_curve_warns() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"curve": [1.0, -3.0, 1.0]}]
        });
        let r = linter().lint(&p);
        assert!(r.warnings.iter().any(|w| w.code == "W002"));
    }

    #[test]
    fn monotonic_curve_no_warning() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"curve": [0.0, 1.0]}]
        });
        let r = linter().lint(&p);
        assert!(!r.warnings.iter().any(|w| w.code == "W002"));
    }

    #[test]
    fn unused_device_reference() {
        let p = json!({
            "name": "t", "version": "1",
            "devices": [{"id": "dev1"}, {"id": "dev2"}],
            "axes": [{"device_id": "dev1"}]
        });
        let r = linter().lint(&p);
        assert!(
            r.warnings
                .iter()
                .any(|w| w.code == "W003" && w.message.contains("dev2"))
        );
        assert!(
            !r.warnings
                .iter()
                .any(|w| w.code == "W003" && w.message.contains("dev1"))
        );
    }

    #[test]
    fn custom_rule() {
        let mut lint = ProfileLinter::new();
        lint.add_rule(LintRule {
            code: "C001".into(),
            description: "custom".into(),
            priority: 0,
            checker: Box::new(|_| {
                (
                    vec![LintWarning {
                        code: "C001".into(),
                        message: "custom warning".into(),
                        path: ".".into(),
                        severity: Severity::Warning,
                        fix: None,
                    }],
                    Vec::new(),
                )
            }),
        });
        let r = lint.lint(&json!({"name":"t","version":"1"}));
        assert!(r.warnings.iter().any(|w| w.code == "C001"));
    }

    // ═══════════════════════════════════════════════════════════════════
    //  NEW DEPTH TESTS
    // ═══════════════════════════════════════════════════════════════════

    // ── 1. Built-in rules ────────────────────────────────────────────

    #[test]
    fn deadzone_negative_value_warns() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"deadzone": -5.0}]
        });
        let r = linter().lint(&p);
        let w = r.warnings.iter().find(|w| w.code == "W001").unwrap();
        assert!(w.message.contains("-5"));
    }

    #[test]
    fn deadzone_boundary_zero_ok() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"deadzone": 0.0}]
        });
        let r = linter().lint(&p);
        assert!(!r.warnings.iter().any(|w| w.code == "W001"));
    }

    #[test]
    fn deadzone_boundary_fifty_ok() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"deadzone": 50.0}]
        });
        let r = linter().lint(&p);
        assert!(!r.warnings.iter().any(|w| w.code == "W001"));
    }

    #[test]
    fn expo_in_range_no_warning() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"expo": 0.5}]
        });
        let r = linter().lint(&p);
        assert!(!r.warnings.iter().any(|w| w.code == "W004"));
    }

    #[test]
    fn expo_out_of_range_warns() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"expo": 2.5}]
        });
        let r = linter().lint(&p);
        assert!(r.warnings.iter().any(|w| w.code == "W004"));
    }

    #[test]
    fn expo_negative_out_of_range_warns() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"expo": -1.5}]
        });
        let r = linter().lint(&p);
        let w = r.warnings.iter().find(|w| w.code == "W004").unwrap();
        assert!(w.message.contains("-1.5"));
    }

    #[test]
    fn expo_boundary_negative_one_ok() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"expo": -1.0}]
        });
        let r = linter().lint(&p);
        assert!(!r.warnings.iter().any(|w| w.code == "W004"));
    }

    #[test]
    fn axis_name_valid_alphanumeric() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"assignment": "roll_axis_1"}]
        });
        let r = linter().lint(&p);
        assert!(!r.errors.iter().any(|e| e.code == "E003"));
    }

    #[test]
    fn axis_name_invalid_spaces() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"assignment": "roll axis"}]
        });
        let r = linter().lint(&p);
        assert!(r.errors.iter().any(|e| e.code == "E003"));
    }

    #[test]
    fn axis_name_invalid_special_chars() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"assignment": "roll-axis!"}]
        });
        let r = linter().lint(&p);
        let e = r.errors.iter().find(|e| e.code == "E003").unwrap();
        assert!(e.message.contains("roll-axis!"));
    }

    #[test]
    fn axis_name_empty_string_invalid() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"assignment": ""}]
        });
        let r = linter().lint(&p);
        assert!(r.errors.iter().any(|e| e.code == "E003"));
    }

    #[test]
    fn duplicate_axis_reports_correct_index() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [
                {"assignment": "pitch"},
                {"assignment": "roll"},
                {"assignment": "pitch"}
            ]
        });
        let r = linter().lint(&p);
        let e = r.errors.iter().find(|e| e.code == "E001").unwrap();
        assert_eq!(e.path, "axes[2].assignment");
    }

    #[test]
    fn conflicting_curve_and_expo_warns() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"curve": [0.0, 1.0], "expo": 0.5}]
        });
        let r = linter().lint(&p);
        assert!(r.warnings.iter().any(|w| w.code == "W005"));
    }

    #[test]
    fn curve_only_no_conflict_warning() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"curve": [0.0, 1.0]}]
        });
        let r = linter().lint(&p);
        assert!(!r.warnings.iter().any(|w| w.code == "W005"));
    }

    #[test]
    fn schema_version_valid_dotted() {
        let p = json!({"name": "t", "version": "1.0"});
        let r = linter().lint(&p);
        assert!(!r.errors.iter().any(|e| e.code == "E004"));
    }

    #[test]
    fn schema_version_valid_single_digit() {
        let p = json!({"name": "t", "version": "1"});
        let r = linter().lint(&p);
        assert!(!r.errors.iter().any(|e| e.code == "E004"));
    }

    #[test]
    fn schema_version_invalid_alpha() {
        let p = json!({"name": "t", "version": "abc"});
        let r = linter().lint(&p);
        assert!(r.errors.iter().any(|e| e.code == "E004"));
    }

    #[test]
    fn schema_version_invalid_trailing_dot() {
        let p = json!({"name": "t", "version": "1."});
        let r = linter().lint(&p);
        assert!(r.errors.iter().any(|e| e.code == "E004"));
    }

    // ── 2. Error reporting ───────────────────────────────────────────

    #[test]
    fn error_location_contains_field_path() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"deadzone": 99.0}]
        });
        let r = linter().lint(&p);
        let w = r.warnings.iter().find(|w| w.code == "W001").unwrap();
        assert_eq!(w.path, "axes[0].deadzone");
    }

    #[test]
    fn severity_levels_correct_for_errors() {
        let p = json!({});
        let r = linter().lint(&p);
        for e in &r.errors {
            assert_eq!(e.severity, Severity::Error);
        }
    }

    #[test]
    fn severity_levels_correct_for_warnings() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"deadzone": 99.0}]
        });
        let r = linter().lint(&p);
        let w = r.warnings.iter().find(|w| w.code == "W001").unwrap();
        assert_eq!(w.severity, Severity::Warning);
    }

    #[test]
    fn error_codes_are_unique_per_rule() {
        let lint = linter();
        let codes = lint.rule_codes();
        let unique: HashSet<&str> = codes.iter().copied().collect();
        assert_eq!(codes.len(), unique.len());
    }

    #[test]
    fn human_readable_messages_non_empty() {
        let p = json!({});
        let r = linter().lint(&p);
        for e in &r.errors {
            assert!(!e.message.is_empty());
            assert!(e.message.len() > 5, "Message too short: '{}'", e.message);
        }
    }

    #[test]
    fn machine_parseable_output_format() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"deadzone": 99.0}]
        });
        let r = linter().lint(&p);
        let output = r.to_machine_readable();
        // Each line: code:severity:path:message
        for line in output.lines() {
            let parts: Vec<&str> = line.splitn(4, ':').collect();
            assert_eq!(parts.len(), 4, "Bad line format: {line}");
            assert!(
                ["info", "warning", "error"].contains(&parts[1]),
                "Bad severity: {}",
                parts[1]
            );
        }
    }

    // ── 3. Auto-fix suggestions ──────────────────────────────────────

    #[test]
    fn deadzone_out_of_range_has_fix_suggestion() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"deadzone": 75.0}]
        });
        let r = linter().lint(&p);
        let w = r.warnings.iter().find(|w| w.code == "W001").unwrap();
        let fix = w.fix.as_ref().unwrap();
        assert_eq!(fix.suggested_value, Some(Value::from(50.0)));
    }

    #[test]
    fn expo_out_of_range_fix_clamps_to_boundary() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"expo": 3.0}]
        });
        let r = linter().lint(&p);
        let w = r.warnings.iter().find(|w| w.code == "W004").unwrap();
        let fix = w.fix.as_ref().unwrap();
        assert_eq!(fix.suggested_value, Some(Value::from(1.0)));
    }

    #[test]
    fn missing_name_fix_suggests_default() {
        let p = json!({"version": "1"});
        let r = linter().lint(&p);
        let e = r.errors.iter().find(|e| e.path == "name").unwrap();
        let fix = e.fix.as_ref().unwrap();
        assert_eq!(fix.suggested_value, Some(Value::String("unnamed".into())));
    }

    #[test]
    fn missing_version_fix_suggests_default() {
        let p = json!({"name": "t"});
        let r = linter().lint(&p);
        let e = r.errors.iter().find(|e| e.path == "version").unwrap();
        let fix = e.fix.as_ref().unwrap();
        assert_eq!(fix.suggested_value, Some(Value::String("1.0".into())));
    }

    #[test]
    fn deprecated_field_has_migration_fix() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"sensitivity": 0.5}]
        });
        let r = linter().lint(&p);
        let w = r.warnings.iter().find(|w| w.code == "W006").unwrap();
        assert_eq!(w.severity, Severity::Info);
        let fix = w.fix.as_ref().unwrap();
        assert!(fix.description.contains("expo"));
    }

    #[test]
    fn auto_fix_clamps_deadzone() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"deadzone": 75.0}]
        });
        let fixed = linter().auto_fix(&p);
        let dz = fixed["axes"][0]["deadzone"].as_f64().unwrap();
        assert!((dz - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn auto_fix_adds_missing_name() {
        let p = json!({"version": "1"});
        let fixed = linter().auto_fix(&p);
        assert_eq!(fixed["name"].as_str(), Some("unnamed"));
    }

    // ── 4. Custom rules ─────────────────────────────────────────────

    #[test]
    fn custom_rule_with_priority_runs_in_order() {
        let mut lint = ProfileLinter::new();
        let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let o1 = order.clone();
        let o2 = order.clone();
        lint.add_rule(LintRule {
            code: "X002".into(),
            description: "second".into(),
            priority: 100,
            checker: Box::new(move |_| {
                o2.lock().unwrap().push("X002");
                (Vec::new(), Vec::new())
            }),
        });
        lint.add_rule(LintRule {
            code: "X001".into(),
            description: "first".into(),
            priority: -10,
            checker: Box::new(move |_| {
                o1.lock().unwrap().push("X001");
                (Vec::new(), Vec::new())
            }),
        });
        lint.lint(&json!({"name":"t","version":"1"}));
        let executed = order.lock().unwrap();
        let x1_pos = executed.iter().position(|c| *c == "X001").unwrap();
        let x2_pos = executed.iter().position(|c| *c == "X002").unwrap();
        assert!(x1_pos < x2_pos, "X001 (prio -10) should run before X002 (prio 100)");
    }

    #[test]
    fn disable_rule_suppresses_findings() {
        let mut lint = linter();
        lint.disable_rule("W001");
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"deadzone": 99.0}]
        });
        let r = lint.lint(&p);
        assert!(!r.warnings.iter().any(|w| w.code == "W001"));
    }

    #[test]
    fn enable_rule_restores_findings() {
        let mut lint = linter();
        lint.disable_rule("W001");
        lint.enable_rule("W001");
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"deadzone": 99.0}]
        });
        let r = lint.lint(&p);
        assert!(r.warnings.iter().any(|w| w.code == "W001"));
    }

    #[test]
    fn disable_multiple_rules() {
        let mut lint = linter();
        lint.disable_rule("E001");
        lint.disable_rule("E002");
        let p = json!({
            "axes": [{"assignment": "roll"}, {"assignment": "roll"}]
        });
        let r = lint.lint(&p);
        assert!(!r.errors.iter().any(|e| e.code == "E001"));
        assert!(!r.errors.iter().any(|e| e.code == "E002"));
    }

    #[test]
    fn custom_rule_configuration_via_closure() {
        let threshold = 10.0_f64;
        let mut lint = linter();
        lint.add_rule(LintRule {
            code: "C002".into(),
            description: "configurable threshold".into(),
            priority: 50,
            checker: Box::new(move |profile| {
                let mut warnings = Vec::new();
                if let Some(axes) = profile.get("axes").and_then(Value::as_array) {
                    for (i, axis) in axes.iter().enumerate() {
                        if let Some(dz) = axis.get("deadzone").and_then(Value::as_f64)
                            && dz > threshold
                        {
                            warnings.push(LintWarning {
                                code: "C002".into(),
                                message: format!("Deadzone {dz} exceeds threshold {threshold}"),
                                path: format!("axes[{i}].deadzone"),
                                severity: Severity::Info,
                                fix: None,
                            });
                        }
                    }
                }
                (warnings, Vec::new())
            }),
        });
        let p = json!({"name":"t","version":"1","axes":[{"deadzone": 15.0}]});
        let r = lint.lint(&p);
        assert!(r.warnings.iter().any(|w| w.code == "C002"));
    }

    // ── 5. Property tests ────────────────────────────────────────────

    #[test]
    fn valid_profile_never_triggers_errors() {
        // A well-formed profile with all valid values produces no errors.
        let profiles = vec![
            json!({"name": "a", "version": "1", "axes": [], "devices": []}),
            json!({"name": "b", "version": "2.0", "axes": [{"assignment": "roll", "deadzone": 5.0}]}),
            json!({"name": "c", "version": "1.0.0", "axes": [{"assignment": "pitch", "expo": 0.3}]}),
        ];
        let lint = linter();
        for p in &profiles {
            let r = lint.lint(p);
            assert!(r.errors.is_empty(), "Unexpected errors for: {p}");
        }
    }

    #[test]
    fn linter_is_idempotent() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"deadzone": 75.0, "assignment": "roll"}, {"assignment": "roll"}]
        });
        let lint = linter();
        let r1 = lint.lint(&p);
        let r2 = lint.lint(&p);
        assert_eq!(r1.warnings.len(), r2.warnings.len());
        assert_eq!(r1.errors.len(), r2.errors.len());
        for (a, b) in r1.warnings.iter().zip(r2.warnings.iter()) {
            assert_eq!(a.code, b.code);
            assert_eq!(a.path, b.path);
        }
    }

    #[test]
    fn auto_fix_produces_valid_profile() {
        let p = json!({
            "axes": [{"deadzone": 99.0, "expo": 5.0}]
        });
        let lint = linter();
        let fixed = lint.auto_fix(&p);
        // The fixed profile should have name + version + clamped values
        assert!(fixed.get("name").is_some());
        assert!(fixed.get("version").is_some());
        let dz = fixed["axes"][0]["deadzone"].as_f64().unwrap();
        assert!((0.0..=50.0).contains(&dz));
        let expo = fixed["axes"][0]["expo"].as_f64().unwrap();
        assert!((-1.0..=1.0).contains(&expo));
    }

    #[test]
    fn auto_fix_idempotent() {
        let p = json!({
            "axes": [{"deadzone": 99.0}]
        });
        let lint = linter();
        let fixed1 = lint.auto_fix(&p);
        let fixed2 = lint.auto_fix(&fixed1);
        assert_eq!(fixed1, fixed2, "Second auto-fix should produce same result");
    }

    #[test]
    fn auto_fix_preserves_valid_fields() {
        let p = json!({
            "name": "my_profile",
            "version": "2.0",
            "axes": [{"deadzone": 10.0, "assignment": "yaw"}],
            "custom_field": "keep_me"
        });
        let fixed = linter().auto_fix(&p);
        assert_eq!(fixed["name"], "my_profile");
        assert_eq!(fixed["version"], "2.0");
        assert_eq!(fixed["custom_field"], "keep_me");
        assert_eq!(fixed["axes"][0]["deadzone"].as_f64().unwrap(), 10.0);
    }

    // ── 6. Additional edge-case depth tests ──────────────────────────

    #[test]
    fn is_clean_on_valid_profile() {
        let r = linter().lint(&valid_profile());
        assert!(r.is_clean());
    }

    #[test]
    fn is_clean_false_on_errors() {
        let r = linter().lint(&json!({}));
        assert!(!r.is_clean());
    }

    #[test]
    fn multiple_axes_multiple_violations() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [
                {"deadzone": -1.0, "expo": 5.0},
                {"deadzone": 80.0, "expo": -2.0}
            ]
        });
        let r = linter().lint(&p);
        let dz_warnings: Vec<_> = r.warnings.iter().filter(|w| w.code == "W001").collect();
        let expo_warnings: Vec<_> = r.warnings.iter().filter(|w| w.code == "W004").collect();
        assert_eq!(dz_warnings.len(), 2);
        assert_eq!(expo_warnings.len(), 2);
    }

    #[test]
    fn deprecated_response_curve_detected() {
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"response_curve": [0.0, 1.0]}]
        });
        let r = linter().lint(&p);
        let w = r
            .warnings
            .iter()
            .find(|w| w.code == "W006" && w.message.contains("response_curve"))
            .unwrap();
        assert!(w.fix.as_ref().unwrap().description.contains("curve"));
    }

    #[test]
    fn empty_axes_array_no_crash() {
        let p = json!({"name": "t", "version": "1", "axes": []});
        let r = linter().lint(&p);
        assert!(r.is_clean());
    }

    #[test]
    fn no_axes_field_no_crash() {
        let p = json!({"name": "t", "version": "1"});
        let r = linter().lint(&p);
        assert!(r.errors.is_empty());
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    fn valid_axis_name() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{0,15}".prop_map(|s| s)
    }

    fn valid_deadzone() -> impl Strategy<Value = f64> {
        (0.0..=50.0_f64).prop_map(|v| (v * 100.0).round() / 100.0)
    }

    fn valid_expo() -> impl Strategy<Value = f64> {
        (-1.0..=1.0_f64).prop_map(|v| (v * 100.0).round() / 100.0)
    }

    proptest! {
        #[test]
        fn prop_valid_profile_no_errors(
            name in "[a-z]{3,10}",
            dz in valid_deadzone(),
            expo in valid_expo(),
            axis_name in valid_axis_name(),
        ) {
            let p = serde_json::json!({
                "name": name,
                "version": "1.0",
                "axes": [{"assignment": axis_name, "deadzone": dz, "expo": expo}],
                "devices": []
            });
            let r = ProfileLinter::new().lint(&p);
            prop_assert!(r.errors.is_empty(), "Unexpected errors: {:?}", r.errors.iter().map(|e| &e.message).collect::<Vec<_>>());
        }

        #[test]
        fn prop_linter_idempotent(
            dz in -100.0..200.0_f64,
            expo in -5.0..5.0_f64,
        ) {
            let p = serde_json::json!({
                "name": "t", "version": "1",
                "axes": [{"deadzone": dz, "expo": expo, "assignment": "roll"}]
            });
            let lint = ProfileLinter::new();
            let r1 = lint.lint(&p);
            let r2 = lint.lint(&p);
            prop_assert_eq!(r1.warnings.len(), r2.warnings.len());
            prop_assert_eq!(r1.errors.len(), r2.errors.len());
        }

        #[test]
        fn prop_auto_fix_produces_valid(
            dz in -100.0..200.0_f64,
            expo in -5.0..5.0_f64,
        ) {
            let p = serde_json::json!({
                "axes": [{"deadzone": dz, "expo": expo}]
            });
            let lint = ProfileLinter::new();
            let fixed = lint.auto_fix(&p);
            let r = lint.lint(&fixed);
            // After fix, no deadzone or expo range warnings should remain
            prop_assert!(!r.warnings.iter().any(|w| w.code == "W001"),
                "W001 still present after fix");
            prop_assert!(!r.warnings.iter().any(|w| w.code == "W004"),
                "W004 still present after fix");
        }
    }
}
