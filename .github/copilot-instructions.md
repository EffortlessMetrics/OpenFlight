# OpenFlight (Flight Hub) — Copilot Instructions

PC flight simulation input management system in Rust. Provides unified axis/panel/FFB control for MSFS, X-Plane, and DCS with a hard real-time 250Hz processing spine.

**Rust 2024 edition · MSRV 1.92.0 · Workspace monorepo**

---

## Build, Test & Lint

```bash
# Build
cargo build --workspace
cargo build --release --workspace
cargo build --profile rt --workspace   # RT-optimized with debug symbols

# Test
cargo test --workspace
cargo test -p flight-core              # Single crate
cargo test -p flight-core my_test      # Single test
cargo test -- --nocapture              # With stdout

# Lint
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings

# Security
cargo audit --deny warnings
cargo deny check
```

### xtask shortcuts

```bash
cargo xtask check      # Fast smoke test (fmt + clippy + core tests)
cargo xtask validate   # Full validation (tests, benches, API checks)
```

### Makefile targets

```bash
make quick             # clippy-strict + verify-patterns (fast dev loop)
make all               # Full regression prevention
make ci-simulation     # Full CI pipeline locally
make clippy-strict     # Strict clippy on the 8 core crates
make feature-powerset  # cargo-hack feature powerset check
make verify-patterns   # Check for banned code patterns (see below)
```

### FFB safety tests (run before touching force-feedback code)

```bash
cargo test -p flight-ffb safety
cargo test -p flight-ffb envelope
```

---

## Architecture

### RT Spine (ADR-001)

```
Non-RT: Sim Adapters │ Panels │ Diagnostics │ IPC
              │  drop-tail lock-free queues  │
RT Spine (250Hz): Axis Engine │ FFB Engine │ Scheduler
```

- The RT spine **never blocks, allocates, or takes locks** (see ADR-004).
- Config changes are compiled off-thread and swapped atomically at tick boundaries.
- Platform RT scheduling: **MMCSS** on Windows, **rtkit** on Linux.

### Crate groups

| Group | Key crates |
|---|---|
| **RT Core** | `flight-axis`, `flight-scheduler`, `flight-bus`, `flight-blackbox` |
| **Hardware** | `flight-hid`, `flight-ffb`, `flight-panels` (+`-core`/`-saitek`/`-cougar`), `flight-streamdeck`, `flight-tactile`, `flight-virtual` |
| **Sim adapters** | `flight-simconnect` (+`-sys`), `flight-xplane`, `flight-dcs-export`, `flight-adapter-common` |
| **Infrastructure** | `flight-core`, `flight-ipc`, `flight-profile`, `flight-rules`, `flight-writers`, `flight-units`, `flight-tracing`, `flight-metrics`, `flight-session`, `flight-updater`, `flight-watchdog`, `flight-security` |
| **Applications** | `flight-service` (daemon `flightd`), `flight-cli` (`flightctl`), `flight-ui` |

IPC uses **gRPC** (tonic 0.14 / prost 0.14). Protobuf sources live in `crates/flight-ipc/proto/`.

### Profile pipeline ownership (ADR-007)

Profiles cascade: **Global → Simulator → Aircraft → Phase-of-Flight**. More-specific profiles override less-specific ones. Merging happens off-thread; the compiled result is atomically swapped into the RT spine. Use `Profile::merge_with` — never `Profile::merge`.

### Writers as Data (ADR-002)

Sim variable configurations are JSON diff tables, not code. Each sim version has a versioned diff that is validated with golden-file tests in CI.

### Plugin tiers (ADR-003)

1. **WASM** — sandboxed, capability-declared, 20–120 Hz  
2. **Native fast-path** — isolated helper process, shared-memory SPSC, per-tick budget  
3. **Service** — managed thread, event-driven, full access with user consent  

---

## Key Conventions

### Zero-allocation in RT code (ADR-004)

These crates have a strict no-allocation rule on hot paths: `flight-axis`, `flight-scheduler`, `flight-ffb`.

**Forbidden:**
- `Box::new()`, `Vec::push()` beyond capacity, any `String` allocation
- `HashMap::insert()` that triggers rehash
- `Arc::new()`, `Rc::new()`
- Blocking syscalls or mutex locks

**Allowed:** stack allocation, pre-allocated containers, atomics, static data.

### Banned patterns (`make verify-patterns` enforces these)

| ❌ Banned | ✅ Use instead |
|---|---|
| `Profile::merge(` | `Profile::merge_with` |
| `criterion::black_box` | `std::hint::black_box` |
| `BlackboxWriter::new(...)?` | (no `?` on this constructor) |
| Per-crate `tokio`/`futures`/`tonic` version pins | `{ workspace = true }` |

### Workspace dependencies

All shared dependencies **must** use `workspace = true` in crate `Cargo.toml` files. Version pins belong in the root `Cargo.toml` workspace table only.

### Strict clippy crates

These 8 crates must pass `cargo clippy -p <crate> -- -D warnings` with zero warnings:

> `flight-core` `flight-axis` `flight-bus` `flight-hid` `flight-ipc` `flight-service` `flight-simconnect` `flight-panels`

### Clippy thresholds (clippy.toml)

- cognitive complexity: 30
- too-many-arguments: 8
- type complexity: 250

---

## Quality Gates

| Gate | Requirement | Runner |
|---|---|---|
| QG-SANITY-GATE | Compiles + fmt clean | Any |
| QG-FFB-SAFETY | All FFB safety/envelope tests pass | Any |
| QG-RT-JITTER | p99 jitter ≤ 0.5 ms | Bare-metal hardware |
| QG-HID-LATENCY | HID write p99 ≤ 300 µs | Hardware + HID device |

Hardware gates are only enforced on `release/*` branches. All non-hardware gates must pass on PRs to `main`.

---

## Documentation

Follows the **Diataxis** framework:

- `docs/explanation/adr/` — Architecture Decision Records (ADR-001 through ADR-011)
- `docs/how-to/` — Task-oriented guides
- `docs/reference/` — API and spec reference
- `docs/NOW_NEXT_LATER.md` — **Current priorities** — check this before picking up tasks

---

## Branching

`main` · `feat/<name>` · `fix/<name>` · `docs/<name>`

**Before submitting a PR:** run `cargo xtask validate` (or `make all`) and verify strict clippy passes on the 8 core crates.
