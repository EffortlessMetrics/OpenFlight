---
doc_id: DOC-REF-CONFIGURATION
title: "Profile Configuration Reference"
status: active
category: reference
group: flight-profile
requirements:
  - REQ-1
  - REQ-2
adrs:
  - ADR-007
---

# Profile Configuration Reference

OpenFlight profiles define how raw device input is transformed before
reaching the simulator. Profiles are YAML files validated against the
`flight.profile/1` schema.

## Schema Version

Every profile file must declare its schema:

```yaml
schema: "flight.profile/1"
```

## Top-Level Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `schema` | string | **yes** | Must be `"flight.profile/1"` |
| `sim` | string | no | Target simulator (`msfs`, `xplane`, `dcs`, …) |
| `aircraft` | string | no | Aircraft identifier for per-aircraft overrides |
| `axes` | map | no | Per-axis configuration (keyed by axis name) |
| `pof_overrides` | map | no | Phase-of-flight overrides |

## Profile Cascade

Profiles merge in a fixed hierarchy — more-specific profiles override
less-specific ones:

```
Global  →  Simulator  →  Aircraft  →  Phase-of-Flight
```

### Merge Rules (ADR-007)

| Value type | Strategy |
|------------|----------|
| Scalars (numbers, strings) | Last writer wins |
| Arrays | Key-merge (matched by key field, e.g., axis name) |
| Curves | Monotonicity preserved; full replacement |
| Detents | No overlap allowed; validated at merge time |

> **Important:** Always use `Profile::merge_with` — `Profile::merge`
> is deprecated and will trigger a CI failure.

## Axis Configuration

Each axis is keyed by its logical name (`x`, `y`, `z`, `rx`, `ry`,
`rz`, `slider`, or a custom name).

```yaml
axes:
  x:
    deadzone:
      center: 0.05
      edge: 0.02
    expo: 0.3
    slew_rate: 50.0
    curve:
      - { x: 0.0, y: 0.0 }
      - { x: 0.5, y: 0.3 }
      - { x: 1.0, y: 1.0 }
    detents:
      - position: 0.0
        width: 0.05
        label: "idle"
      - position: 1.0
        width: 0.03
        label: "toga"
    filter:
      alpha: 0.15
      spike_threshold: 0.4
      max_spike_count: 3
```

### Deadzone

Controls the dead region around axis center and edges.

| Field | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `center` | f32 | 0.0 – 0.5 | 0.0 | Values within this radius of center are collapsed to 0 |
| `edge` | f32 | 0.0 – 0.5 | 0.0 | Values within this distance of ±1.0 are saturated to ±1.0 |

The remaining range is linearly rescaled so that the first value
outside the deadzone maps smoothly from zero to full deflection.

### Expo (Exponential Curve)

| Field | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `expo` | f32 | 0.0 – 1.0 | 0.0 | Blends between linear (0) and cubic (1) response |

`expo: 0.0` → linear response.
`expo: 1.0` → maximum sensitivity reduction near center, maximum
sensitivity at extremes.

### Slew Rate

| Field | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `slew_rate` | f32 | 0.0 – 100.0 | 100.0 | Maximum change per second (normalised units/s) |

Limits how quickly the output can change between ticks. Useful for
smoothing throttle inputs.

### Custom Response Curve

Define arbitrary input-to-output mappings with control points:

```yaml
curve:
  - { x: 0.0, y: 0.0 }
  - { x: 0.25, y: 0.1 }
  - { x: 0.5, y: 0.35 }
  - { x: 0.75, y: 0.7 }
  - { x: 1.0, y: 1.0 }
```

| Field | Type | Range | Description |
|-------|------|-------|-------------|
| `x` | f32 | 0.0 – 1.0 | Input value (normalised) |
| `y` | f32 | 0.0 – 1.0 | Output value (normalised) |

Points are interpolated using one of three modes:

| Mode | Description |
|------|-------------|
| `Linear` | Straight-line segments between points |
| `CubicHermite` | Smooth cubic interpolation |
| `MonotoneCubic` | Cubic with guaranteed monotonicity |

The interpolation mode is set at the `ResponseCurve` level. Curves
must be monotonically non-decreasing in `x`.

### Detents

Detents are "sticky" positions on a linear axis — useful for throttle
gates (idle, climb, flex, TOGA).

```yaml
detents:
  - position: 0.0
    width: 0.05
    label: "idle"
  - position: 0.65
    width: 0.04
    label: "climb"
  - position: 0.89
    width: 0.03
    label: "flex"
  - position: 1.0
    width: 0.03
    label: "toga"
```

| Field | Type | Description |
|-------|------|-------------|
| `position` | f32 | Centre of the detent (0.0 – 1.0) |
| `width` | f32 | Half-width of the snap region |
| `label` | string | Human-readable name |

When the raw input enters the detent's snap region
(`position ± width`), the output snaps to `position`. Hysteresis
prevents oscillation at the boundary.

**Predefined presets:**

- `DetentConfig::standard_throttle()` — idle, climb, TOGA
- `DetentConfig::airbus_throttle()` — idle, climb, flex, TOGA

### EMA Filter

An exponential moving average filter for smoothing noisy
potentiometer inputs:

| Field | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `alpha` | f32 | 0.0 – 1.0 | 1.0 | Smoothing factor (lower = smoother, higher = responsive) |
| `spike_threshold` | f32 | 0.0 – 1.0 | 1.0 | Changes larger than this are treated as spikes |
| `max_spike_count` | u32 | ≥ 1 | 3 | Consecutive spikes before accepting the new value |

## Button Mapping

Buttons can be mapped to simulator actions or internal commands:

```yaml
buttons:
  1:
    action: "sim.gear_toggle"
    mode: "press"          # press | release | toggle | hold
  2:
    action: "sim.flaps_up"
    mode: "press"
  3:
    action: "sim.flaps_down"
    mode: "press"
```

| Field | Type | Description |
|-------|------|-------------|
| `action` | string | Target action (namespace.action format) |
| `mode` | string | Trigger mode: `press`, `release`, `toggle`, `hold` |

## Per-Aircraft Overrides

Bind a profile to a specific aircraft:

```yaml
schema: "flight.profile/1"
sim: "msfs"
aircraft: "Fenix A320"

axes:
  z:
    detents:
      - position: 0.0
        width: 0.05
        label: "idle"
      - position: 0.65
        width: 0.04
        label: "climb"
      - position: 0.89
        width: 0.03
        label: "flex"
      - position: 1.0
        width: 0.03
        label: "toga"
```

This profile only activates when the `Fenix A320` is detected.
It merges on top of any simulator-level and global profiles.

## Phase-of-Flight Overrides

Adjust axis behaviour based on flight phase:

```yaml
pof_overrides:
  takeoff:
    axes:
      x:
        expo: 0.5
        slew_rate: 80.0
  cruise:
    axes:
      x:
        expo: 0.2
        slew_rate: 30.0
  landing:
    axes:
      x:
        expo: 0.4
```

Phase detection relies on telemetry from the active simulator
adapter (gear state, altitude, speed, flap position). Hysteresis
prevents rapid toggling between phases.

## Capability Modes

Profiles can be restricted by capability mode:

| Mode | Description |
|------|-------------|
| `Full` | All features enabled (default) |
| `Demo` | Limited feature set for demonstrations |
| `Kid` | Restricted inputs, reduced sensitivity |

Capability limits are enforced by `CapabilityContext` and
`CapabilityLimits` in `flight-profile`.

## Validation

Profiles are validated at load time. Common validation errors:

| Error | Cause |
|-------|-------|
| Invalid schema version | `schema` is not `"flight.profile/1"` |
| Deadzone out of range | `center` or `edge` > 0.5 |
| Curve not monotonic | `x` values are not strictly increasing |
| Detent overlap | Two detents' snap regions intersect |
| Unknown axis name | Axis name does not match device capabilities |

### Deterministic Canonicalisation

For hashing and change detection, profiles are canonicalised:

- Floats are normalised to 6 decimal places
- Keys are sorted lexicographically
- Whitespace is normalised

This ensures identical logical profiles produce identical hashes
regardless of formatting.

## Complete Example

```yaml
schema: "flight.profile/1"
sim: "msfs"
aircraft: "PMDG 737-800"

axes:
  x:
    deadzone:
      center: 0.04
      edge: 0.02
    expo: 0.25
    curve:
      - { x: 0.0, y: 0.0 }
      - { x: 0.5, y: 0.3 }
      - { x: 1.0, y: 1.0 }
    filter:
      alpha: 0.2
      spike_threshold: 0.5
      max_spike_count: 3

  y:
    deadzone:
      center: 0.04
      edge: 0.02
    expo: 0.25

  z:
    deadzone:
      center: 0.08
    slew_rate: 40.0
    detents:
      - position: 0.0
        width: 0.05
        label: "idle"
      - position: 0.65
        width: 0.04
        label: "climb"
      - position: 1.0
        width: 0.03
        label: "toga"

pof_overrides:
  takeoff:
    axes:
      x:
        expo: 0.5
  cruise:
    axes:
      x:
        expo: 0.15

buttons:
  1:
    action: "sim.gear_toggle"
    mode: "press"
  2:
    action: "sim.flaps_up"
    mode: "press"
  3:
    action: "sim.flaps_down"
    mode: "press"
```

## See Also

- [Getting Started](../how-to/getting-started.md) — creating your
  first profile
- [Architecture Overview](architecture-overview.md) — how profiles
  are compiled and swapped into the RT spine
- [ADR-007](../explanation/adr/007-pipeline-ownership-model.md) —
  pipeline ownership and merge semantics
