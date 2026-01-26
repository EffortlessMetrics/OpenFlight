# Contributing to OpenFlight

Thank you for your interest in contributing! We welcome contributions from everyone.

## Development Workflow

### 1. "Now/Next/Later" Priorities
We use [docs/NOW_NEXT_LATER.md](docs/NOW_NEXT_LATER.md) to track immediate focus. Please check this before picking up new tasks to ensure alignment with current goals.

### 2. Validation Pipeline
All changes must pass the validation pipeline:
```bash
cargo xtask validate
```

### 3. Documentation
We follow the **Diataxis** framework. When adding features:
- **Tutorials**: Learning-oriented (e.g., "Getting Started").
- **How-To**: Problem-oriented (e.g., "How to integrate XInput").
- **Explanation**: Understanding-oriented (e.g., "Concepts").
- **Reference**: Information-oriented (e.g., "API Specs").

See `docs/README.md` for the index.

### 4. Branching Strategy
- `main`: Stable development branch.
- `feat/`: Feature branches.
- `fix/`: Bug fix branches.
- `docs/`: Documentation updates.
