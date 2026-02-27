use serde_json::Value;
use std::collections::HashSet;

/// Result of linting a profile configuration.
pub struct LintResult {
    pub warnings: Vec<LintWarning>,
    pub errors: Vec<LintError>,
}

/// A non-fatal lint finding.
pub struct LintWarning {
    pub code: String,
    pub message: String,
    pub path: String,
}

/// A fatal lint finding.
pub struct LintError {
    pub code: String,
    pub message: String,
    pub path: String,
}

/// A single lint rule applied to profile JSON.
pub struct LintRule {
    pub code: String,
    pub description: String,
    checker: CheckerFn,
}

type CheckerFn = Box<dyn Fn(&Value) -> (Vec<LintWarning>, Vec<LintError>) + Send + Sync>;

/// Static analyser for profile configurations.
pub struct ProfileLinter {
    rules: Vec<LintRule>,
}

impl ProfileLinter {
    /// Creates a linter pre-loaded with all built-in rules.
    #[must_use]
    pub fn new() -> Self {
        let mut linter = Self { rules: Vec::new() };
        linter.add_builtin_rules();
        linter
    }

    /// Registers an additional lint rule.
    pub fn add_rule(&mut self, rule: LintRule) {
        self.rules.push(rule);
    }

    /// Runs every registered rule against `profile_json` and returns the
    /// aggregated warnings and errors.
    pub fn lint(&self, profile_json: &Value) -> LintResult {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        for rule in &self.rules {
            let (w, e) = (rule.checker)(profile_json);
            warnings.extend(w);
            errors.extend(e);
        }
        LintResult { warnings, errors }
    }

    fn add_builtin_rules(&mut self) {
        self.rules.push(Self::rule_overlapping_axes());
        self.rules.push(Self::rule_missing_required_fields());
        self.rules.push(Self::rule_deadzone_range());
        self.rules.push(Self::rule_curve_monotonicity());
        self.rules.push(Self::rule_unused_device_refs());
    }

    // ── Built-in rules ───────────────────────────────────────────────

    fn rule_overlapping_axes() -> LintRule {
        LintRule {
            code: "E001".into(),
            description: "Overlapping axis assignments".into(),
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
            checker: Box::new(|profile| {
                let mut errors = Vec::new();
                let required = ["name", "version"];
                for field in required {
                    if profile.get(field).is_none() {
                        errors.push(LintError {
                            code: "E002".into(),
                            message: format!("Missing required field '{field}'"),
                            path: field.to_owned(),
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
            checker: Box::new(|profile| {
                let mut warnings = Vec::new();
                if let Some(axes) = profile.get("axes").and_then(Value::as_array) {
                    for (i, axis) in axes.iter().enumerate() {
                        if let Some(dz) = axis.get("deadzone").and_then(Value::as_f64)
                            && !(0.0..=50.0).contains(&dz)
                        {
                            warnings.push(LintWarning {
                                code: "W001".into(),
                                message: format!("Deadzone {dz}% is outside the valid 0–50% range"),
                                path: format!("axes[{i}].deadzone"),
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
                        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn linter() -> ProfileLinter {
        ProfileLinter::new()
    }

    #[test]
    fn valid_profile_passes() {
        let p = json!({
            "name": "default",
            "version": "1.0",
            "axes": [{"assignment": "roll", "deadzone": 5.0}],
            "devices": []
        });
        let r = linter().lint(&p);
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
        // coefficients: y = 1 - 3x + x^2 → dips below initial value
        let p = json!({
            "name": "t", "version": "1",
            "axes": [{"curve": [1.0, -3.0, 1.0]}]
        });
        let r = linter().lint(&p);
        assert!(r.warnings.iter().any(|w| w.code == "W002"));
    }

    #[test]
    fn monotonic_curve_no_warning() {
        // y = x  (coefficients [0, 1])
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
            checker: Box::new(|_| {
                (
                    vec![LintWarning {
                        code: "C001".into(),
                        message: "custom warning".into(),
                        path: ".".into(),
                    }],
                    Vec::new(),
                )
            }),
        });
        let r = lint.lint(&json!({"name":"t","version":"1"}));
        assert!(r.warnings.iter().any(|w| w.code == "C001"));
    }
}
