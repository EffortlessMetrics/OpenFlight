---
doc_id: DOC-TUTORIAL-GETTING-STARTED
kind: tutorial
area: infra
status: active
links:
  requirements: ["INF-REQ-8"]
  tasks: []
  adrs: []
---

# Getting Started with OpenFlight

This tutorial will guide you through setting up the OpenFlight development environment and running your first simulated flight integration.

## Prerequisites

*   **Rust**: Version 1.92.0 or later (2024 edition).
*   **Docker**: For running infrastructure services.
*   **Simulator**: MSFS 2020/2024, X-Plane 11/12, or DCS World (optional, but recommended).

## Step 1: Clone and Build

1.  Clone the repository:
    ```bash
    git clone https://github.com/EffortlessMetrics/OpenFlight.git
    cd OpenFlight
    ```

2.  Build the workspace:
    ```bash
    cargo build --release
    ```

## Step 2: Run the Virtual Harness

OpenFlight includes a virtual device harness to test without physical hardware.

1.  Run the virtual flight loop:
    ```bash
    cargo run -p flight-virtual
    ```
    You should see logs indicating the axis engine is running at 250Hz.

## Step 3: Validate Your Environment

Use the `xtask` system to check your setup:

```bash
cargo xtask check
cargo xtask validate-infra
```

## Next Steps

*   Check out the [Concepts](../explanation/README.md) to understand the architecture.
*   See [How-To Guides](../how-to/README.md) for specific integrations.
