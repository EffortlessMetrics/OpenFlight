# Benchmark Baselines

This directory contains JSON baseline files used by `cargo xtask bench-compare`
to detect performance regressions in CI.

## Format

Each baseline file is a JSON object with the following schema:

```json
{
  "version": 1,
  "created_at": "2025-01-15T12:00:00Z",
  "baselines": {
    "<benchmark_name>": {
      "mean_ns": 1234.5,
      "description": "Human-readable description of what this measures"
    }
  }
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `version` | integer | Schema version (currently `1`) |
| `created_at` | string | ISO 8601 timestamp when baselines were captured |
| `baselines` | object | Map of benchmark name → baseline data |
| `baselines.<name>.mean_ns` | float | Expected mean time in nanoseconds |
| `baselines.<name>.description` | string | What the benchmark measures |

## Files

| File | Crate | Critical Path |
|------|-------|---------------|
| `flight-axis.json` | flight-axis | 250Hz axis processing throughput |
| `flight-bus.json` | flight-bus | Event routing and publish/subscribe latency |
| `flight-profile.json` | flight-profile | Profile merge and canonicalization |

## Updating Baselines

To regenerate baselines from current benchmark results:

```bash
cargo xtask bench-compare --save-baseline
```

This runs all tracked benchmarks and writes updated baseline files.

## Regression Threshold

The default regression threshold is **10%**. A benchmark is flagged as a
regression if its mean time exceeds `baseline_mean_ns * (1 + threshold)`.

The threshold can be overridden in CI:

```bash
cargo xtask bench-compare --threshold 15
```
