---
doc_id: DOC-TUTORIAL-FIRST-PROFILE
kind: tutorial
area: profile
status: active
links:
  requirements: ["CORE-REQ-1", "CORE-REQ-2"]
  tasks: []
  adrs: []
---

# Creating Your First Profile

This tutorial teaches you how to write a Flight Hub profile JSON file, validate it, apply it to the running service, and use phase-of-flight overrides to fine-tune axis feel throughout a flight.

By the end you will have:

- A global fallback profile that applies to any aircraft
- An aircraft-specific profile for the Cessna 172 (C172)
- Phase-of-flight overrides that tighten expo on approach and loosen it during cruise
- Confirmed the profile is accepted by `flightctl`

---

## Background

A **profile** is a JSON file that tells Flight Hub how to process axis inputs before they reach the simulator.  Every profile belongs to one level in the cascade:

```
Global → Simulator → Aircraft → Phase-of-Flight
```

More-specific profiles win: an aircraft profile overrides the global one, and a phase-of-flight section overrides the base aircraft settings.  Flight Hub merges them at runtime so you only need to specify what differs.

---

## Prerequisites

- Flight Hub service running (`flightd` or `cargo run -p flight-service`)
- `flightctl` in your `PATH` (or `cargo run -p flight-cli --`)
- A text editor

---

## Step 1 — Write a global profile

Create a file called `global.json`:

```json
{
  "schema": "flight.profile/1",
  "axes": {
    "pitch": {
      "deadzone": 0.03,
      "expo": 0.15,
      "slew_rate": 1.5,
      "detents": []
    },
    "roll": {
      "deadzone": 0.03,
      "expo": 0.15,
      "detents": []
    },
    "yaw": {
      "deadzone": 0.05,
      "expo": 0.10,
      "detents": []
    },
    "throttle": {
      "deadzone": 0.01,
      "expo": 0.05,
      "detents": []
    }
  }
}
```

**Field reference:**

| Field | Range | Effect |
|---|---|---|
| `deadzone` | 0.0 – 0.5 | Ignore stick movement inside this radius |
| `expo` | 0.0 – 1.0 | 0 = linear; higher = more centre softness |
| `slew_rate` | 0.0 – 100.0 | Maximum change per second (smooths snapback) |
| `detents` | list | Click-stop zones (empty = none) |

### Validate it

```bash
flightctl profile apply global.json --validate-only
```

Expected output:

```
✓ Schema valid (flight.profile/1)
✓ 4 axis configurations parsed
✓ All values in range
Profile is valid. Use --validate-only=false to apply.
```

---

## Step 2 — Add an aircraft-specific profile

Some aircraft need tweaked settings.  The C172 has a sensitive pitch axis; create `c172.json`:

```json
{
  "schema": "flight.profile/1",
  "aircraft": {
    "icao": "C172"
  },
  "axes": {
    "pitch": {
      "deadzone": 0.02,
      "expo": 0.10,
      "slew_rate": 1.2,
      "detents": []
    },
    "roll": {
      "deadzone": 0.02,
      "expo": 0.12,
      "detents": []
    },
    "yaw": {
      "deadzone": 0.04,
      "expo": 0.08,
      "detents": []
    }
  }
}
```

> **Tip:** You only need to include axes you want to change.  Any axis absent here inherits from the global profile.

---

## Step 3 — Add phase-of-flight overrides

Approaches require more precision; taxiing needs a restricted yaw range.  Add a `pof_overrides` block to `c172.json`:

```json
{
  "schema": "flight.profile/1",
  "aircraft": {
    "icao": "C172"
  },
  "axes": {
    "pitch": {
      "deadzone": 0.02,
      "expo": 0.10,
      "slew_rate": 1.2,
      "detents": []
    },
    "roll": {
      "deadzone": 0.02,
      "expo": 0.12,
      "detents": []
    },
    "yaw": {
      "deadzone": 0.04,
      "expo": 0.08,
      "detents": []
    }
  },
  "pof_overrides": {
    "approach": {
      "axes": {
        "pitch": {
          "expo": 0.25,
          "deadzone": 0.02,
          "detents": []
        },
        "roll": {
          "expo": 0.20,
          "detents": []
        }
      }
    },
    "cruise": {
      "axes": {
        "pitch": {
          "expo": 0.05,
          "detents": []
        }
      }
    },
    "taxi": {
      "axes": {
        "yaw": {
          "expo": 0.03,
          "deadzone": 0.03,
          "detents": []
        }
      }
    }
  }
}
```

Valid phase-of-flight keys: `takeoff`, `climb`, `cruise`, `descent`, `approach`, `taxi`, `parked`.

---

## Step 4 — Apply both profiles

Apply the global profile first:

```bash
flightctl profile apply global.json
```

Then apply the aircraft profile:

```bash
flightctl profile apply c172.json
```

Flight Hub merges them into the cascade automatically.

### JSON output (for scripting)

```bash
flightctl --json profile apply c172.json
```

Returns structured JSON on success:

```json
{
  "status": "applied",
  "profile_schema": "flight.profile/1",
  "aircraft": "C172",
  "axes_configured": ["pitch", "roll", "yaw"],
  "pof_phases": ["approach", "cruise", "taxi"]
}
```

---

## Step 5 — Verify the active profile

```bash
flightctl profile show
```

You will see the merged effective profile for the currently detected aircraft.  Pass `--raw` to see the full JSON:

```bash
flightctl profile show --raw
```

---

## Step 6 — Check service status

```bash
flightctl status
```

Confirm the service shows `Running` and the expected device count.

---

## What to do next

- Add a **filter** block to reduce potentiometer jitter on a noisy throttle:

  ```json
  "throttle": {
    "deadzone": 0.01,
    "expo": 0.05,
    "detents": [],
    "filter": {
      "alpha": 0.10,
      "spike_threshold": 0.05,
      "max_spike_count": 3
    }
  }
  ```

- Add **detent zones** to mark idle and max-throttle positions:

  ```json
  "throttle": {
    "deadzone": 0.01,
    "expo": 0.05,
    "detents": [
      { "position": 0.0,  "width": 0.03, "role": "idle" },
      { "position": 1.0,  "width": 0.03, "role": "max" }
    ]
  }
  ```

- Learn about **simulator-scoped profiles** — add `"sim": "msfs"` to target a specific simulator.

- See [How-To: Prevent Regressions](../how-to/prevent-regressions.md) to add your profile to the test suite.
