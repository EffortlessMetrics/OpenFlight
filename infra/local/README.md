# Local Development Environment

This directory contains the local development environment configuration for Flight Hub.

## Overview

The local development environment uses Docker Compose to provide a consistent, reproducible development setup with Rust 1.92.0 and edition 2024.

## Prerequisites

- Docker and Docker Compose installed
- Git (for cloning the repository)

## Setup Instructions

1. **Clone the repository** (if not already done):
   ```bash
   git clone <repository-url>
   cd <repository-directory>
   ```

2. **Start the development environment**:
   ```bash
   docker compose -f infra/local/docker-compose.yml up
   ```

   This will:
      - Build the flight-service container using Rust 1.92.0
   - Mount your local workspace into the container at `/workspace`
   - Expose the service on port 8080
   - Start the service with health checks enabled

3. **Verify the environment is running**:
   ```bash
   curl -f http://localhost:8080/health
   ```
   
   Expected response: HTTP 200 status code

## Running the Environment

### Start services in foreground:
```bash
docker compose -f infra/local/docker-compose.yml up
```

### Start services in background (detached mode):
```bash
docker compose -f infra/local/docker-compose.yml up -d
```

### Stop services:
```bash
docker compose -f infra/local/docker-compose.yml down
```

### Rebuild services after dependency changes:
```bash
docker compose -f infra/local/docker-compose.yml up --build
```

## Health Check

The flight-service exposes a health check endpoint at:
```
http://localhost:8080/health
```

You can verify the service is healthy by running:
```bash
curl -f http://localhost:8080/health
```

A successful response (HTTP 200) indicates the service is running correctly.

## Environment Variables

See `invariants.yaml` for the complete list of environment variables and their defaults.

Key environment variables:
- `RUST_LOG`: Logging level (default: "info")
  - Options: trace, debug, info, warn, error
  - Example: `RUST_LOG=debug docker compose up`

## Rapid Iteration

The workspace is bind-mounted into the container, allowing you to:
- Edit code on your host machine
- Changes are immediately visible inside the container
- Rebuild and test without recreating containers

## Troubleshooting

### Port already in use
If port 8080 is already in use, you can either:
1. Stop the conflicting service
2. Modify the port mapping in `docker-compose.yml`

### Container fails to start
Check the logs:
```bash
docker compose -f infra/local/docker-compose.yml logs flight-service
```

### Permission issues
Ensure Docker has permission to access the workspace directory.

## Configuration

The local environment configuration is defined in:
- `docker-compose.yml`: Service definitions and container configuration
- `invariants.yaml`: Environment contracts and requirements

For more details on the environment contracts, see `invariants.yaml`.
