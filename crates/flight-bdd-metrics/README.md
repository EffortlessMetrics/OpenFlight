# flight-bdd-metrics

This microcrate provides reusable BDD coverage and traceability primitives for OpenFlight.

- Parse spec ledgers (YAML) and Gherkin feature scenarios.
- Compute acceptance-criteria, test, and microcrate coverage metrics.
- Normalize crate extraction from test command references.
- Render canonical BDD coverage and microcrate matrix markdown reports.
- Produce standardized coverage status and percentage helpers for report generation.
- Provide microcrate-aware helpers (`is_fully_tested`, `is_fully_gherkin_covered`, `is_fully_both_covered`, and coverage accessors) for downstream quality gates.
