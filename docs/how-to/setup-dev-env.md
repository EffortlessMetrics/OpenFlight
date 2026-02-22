---
doc_id: DOC-HOWTO-SETUP-DEV
kind: how-to
area: infra
status: active
links:
  requirements: [INF-REQ-8]
  tasks: []
  adrs: []
---

# How to Set Up the Development Environment

This guide walks you through setting up a local development environment for Flight Hub.

## Prerequisites

### Required Tools

- **Rust**: Version 1.92.0 or later
- **Docker**: For containerized development (recommended)
- **Git**: For version control

### Optional Tools

- **cargo-public-api**: For API stability checks
- **cargo-nextest**: For faster test execution
- **cargo-watch**: For automatic rebuilds during development

## Quick Start with Docker Compose

The fastest way to get started is using Docker Compose:

```bash
# Clone the repository
git clone https://github.com/your-org/flight-hub.git
cd flight-hub

# Start the development environment
docker compose -f infra/local/docker-compose.yml up
```

This will:
- Build the Flight Hub service with Rust 1.92.0
- Mount your local source code for live editing
- Expose the service on port 8080
- Set up proper environment variables

### Verify the Setup

Check that the service is running:

```bash
curl -f http://localhost:8080/health
```

You should receive a 200 OK response.

## Quick Start with Nix (Linux/macOS)

If you use Nix, a dev shell is provided:

```bash
nix develop
```

This shell includes Rust 1.92.0, `protoc`, `pkg-config`, libusb, and the optional cargo tools used by `cargo xtask validate`.

## Manual Setup (Without Docker)

If you prefer to run directly on your host machine:

### 1. Install Rust

```bash
# Install rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install the required toolchain
rustup install 1.92.0
rustup default 1.92.0
```

### 2. Install System Dependencies

**Windows:**
- Visual Studio 2022 with C++ build tools
- Windows SDK

**Linux (Ubuntu/Debian):**
```bash
sudo apt-get update
sudo apt-get install build-essential pkg-config libusb-1.0-0-dev
```

**macOS:**
```bash
xcode-select --install
brew install libusb
```

### 3. Build the Project

```bash
# Build all crates
cargo build

# Run tests to verify
cargo test
```

### 4. Run the Service

```bash
cargo run -p flight-service
```

## Environment Configuration

### Required Environment Variables

The following environment variables are required:

- `RUST_LOG`: Logging level (default: `info`)
  ```bash
  export RUST_LOG=debug
  ```

### Optional Environment Variables

- `FLIGHT_CONFIG_PATH`: Path to configuration directory
- `FLIGHT_PROFILE_PATH`: Path to profile directory

See `infra/local/invariants.yaml` for the complete list of environment variables and their defaults.

## Development Workflow

### Running Fast Checks

Before committing, run the fast check command:

```bash
cargo xtask check
```

This runs:
- Code formatting checks
- Clippy lints on core crates
- Unit tests for core crates

### Running Full Validation

For comprehensive validation:

```bash
cargo xtask validate
```

This includes everything in `check` plus:
- Benchmarks
- Public API verification
- Cross-reference checks
- Schema validation

### Hot Reloading

When using Docker Compose, your source code is bind-mounted, so changes are immediately available. Rebuild with:

```bash
docker compose -f infra/local/docker-compose.yml restart
```

## IDE Setup

### Visual Studio Code

Recommended extensions:
- `rust-analyzer`: Rust language support
- `CodeLLDB`: Debugging support
- `Even Better TOML`: TOML syntax highlighting

### IntelliJ IDEA / CLion

Install the Rust plugin from JetBrains Marketplace.

## Troubleshooting

### Build Failures

**Issue**: Linker errors on Windows
**Solution**: Ensure Visual Studio C++ build tools are installed

**Issue**: USB library not found on Linux
**Solution**: Install libusb development headers:
```bash
sudo apt-get install libusb-1.0-0-dev
```

### Docker Issues

**Issue**: Permission denied accessing Docker socket
**Solution**: Add your user to the docker group:
```bash
sudo usermod -aG docker $USER
```
Then log out and back in.

**Issue**: Port 8080 already in use
**Solution**: Stop other services using port 8080 or modify the port mapping in `docker-compose.yml`

## Next Steps

- Read the [How to Run Tests](./run-tests.md) guide
- Explore the [Flight Core Concepts](../concepts/flight-core.md)
- Review the [Architecture Decision Records](../adr/README.md)

## Related Requirements

This guide implements **INF-REQ-8: Local Development Environment**, which specifies the requirements for reproducible local development setups.

