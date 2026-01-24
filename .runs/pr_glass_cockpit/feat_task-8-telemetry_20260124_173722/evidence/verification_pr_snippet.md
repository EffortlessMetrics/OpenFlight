
### regression_prevention

`ash
cargo +nightly -Zscript scripts/regression_prevention.rs verify-patterns
``n- Exit code: 1

Head output (tail):
``n🔍 Verifying critical patterns are fixed...
❌ Found unaligned reference warnings:
    Updating crates.io index
error: failed to select a version for `reqwest`.
    ... required by package `flight-xplane v0.1.0 (D:\Code\Rust\OpenFlight\OpenFlight\crates\flight-xplane)`
versions that meet the requirements `^0.13.1` are: 0.13.1

package `flight-xplane` depends on `reqwest` with feature `rustls-tls` but `reqwest` does not have that feature.


failed to select a version for `reqwest` which could resolve this conflict


``n
### perf_dashboard

`ash
cargo +nightly -Zscript scripts/ci_perf_dashboard.rs -- --collect
``n- Exit code: 1

Head output (tail):
``n
warning: function `parse_hid_latency_from_line` is never used
   --> ci_perf_dashboard.rs:385:4
    |
385 | fn parse_hid_latency_from_line(line: &str) -> Option<f64> {
    |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `calculate_percentiles` is never used
   --> ci_perf_dashboard.rs:398:4
    |
398 | fn calculate_percentiles(values: &[f64]) -> (f64, f64) {
    |    ^^^^^^^^^^^^^^^^^^^^^

warning: function `calculate_p99` is never used
   --> ci_perf_dashboard.rs:416:4
    |
416 | fn calculate_p99(values: &[f64]) -> f64 {
    |    ^^^^^^^^^^^^^

Unknown argument: --

``n
